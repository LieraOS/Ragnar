// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-wallpaper — swaybg process management
//
// This service is the single swaybg owner for the session:
//   - one swaybg per enabled output with its mapped image (or the
//     global fallback), spawned via -o <connector>
//   - solid_color mode when no image is configured
// Robustness: children are tracked (and reaped — no zombies), a
// post-spawn check retries once when a spawn dies instantly (e.g.
// the compositor has not advertised the output yet at session
// start), and a watchdog re-applies whenever every swaybg is gone.

use crate::{accounts, background::Choice, map, outputs};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// The currently running swaybg children.
static CURRENT: Mutex<Vec<Child>> = Mutex::new(Vec::new());
/// The last applied choice — the watchdog re-applies it after
/// unexpected deaths.
static LAST: Mutex<Option<Choice>> = Mutex::new(None);

/// Milliseconds old swaybg processes may overlap the new set.
const HANDOVER_MS: u64 = 850;
/// Grace time before a freshly spawned child is checked — a swaybg
/// that fails (unknown output, no compositor) exits inside this
/// window.
const SPAWN_CHECK_MS: u64 = 1500;
/// Watchdog interval: re-applies when any tracked swaybg is gone.
const WATCHDOG_S: u32 = 15;
/// Fast retries after a spawn check finds dead children — covers a
/// multi-second compositor startup window; the slow watchdog keeps
/// retrying afterwards either way.
const MAX_FAST_RETRIES: u32 = 10;
/// Consecutive failed spawn checks — caps the fast retry so a
/// permanently failing swaybg does not loop at full speed (the
/// watchdog still retries every WATCHDOG_S).
static FAILS: AtomicU32 = AtomicU32::new(0);

/// True when the tracked set cannot render anything: empty (all
/// spawns failed) or at least one child already exited. A partial
/// loss counts too — a monitor must never stay without wallpaper.
fn set_is_broken() -> bool {
    let mut current = CURRENT.lock().unwrap();
    current.is_empty()
        || current
            .iter_mut()
            .any(|c| !matches!(c.try_wait(), Ok(None)))
}

/// What to do with the last choice after the watchdog finds any
/// tracked swaybg dead or missing.
pub fn watch_last() {
    glib::timeout_add_local(
        std::time::Duration::from_secs(WATCHDOG_S as u64),
        move || {
            if set_is_broken() {
                if let Some(c) = LAST.lock().unwrap().clone() {
                    apply(&c);
                }
            }
            glib::ControlFlow::Continue
        },
    );
}

/// Applies a wallpaper choice across all enabled outputs.
pub fn apply(choice: &Choice) {
    *LAST.lock().unwrap() = Some(clone_choice(choice));
    let names = outputs::enabled();
    let spawned = match choice {
        Choice::Image { path, mode } => spawn_images(&names, path, mode, &map::load()),
        Choice::SolidColor { color } => spawn_color(color),
    };
    // Reap the previous set (kill + collect so nothing lingers
    // as <defunct> under us).
    retire_children(CURRENT.lock().unwrap().split_off(0));
    *CURRENT.lock().unwrap() = spawned;
    check_spawned();
    if let Choice::Image { path, .. } = choice {
        // Greeter sync must never stall wallpaper management: a slow
        // or hanging AccountsService would park the main loop before
        // the watchdog timers are armed — seen as a futex-blocked
        // daemon, dead swaybg children and a missing wallpaper.
        let p = path.clone();
        std::thread::spawn(move || accounts::set_background(&p));
    }
}

/// Verifies the freshly spawned children after the grace window;
/// if any died already (compositor race), retries the whole apply.
/// The retry budget covers a slow compositor startup; afterwards
/// the watchdog keeps applying at WATCHDOG_S intervals.
fn check_spawned() {
    glib::timeout_add_local_once(
        std::time::Duration::from_millis(SPAWN_CHECK_MS),
        move || {
            if set_is_broken() {
                // Bounded fast retries; the watchdog heals beyond
                // that at its slower pace — no tight respawn loop.
                if FAILS.fetch_add(1, Ordering::Relaxed) < MAX_FAST_RETRIES {
                    if let Some(c) = LAST.lock().unwrap().clone() {
                        apply(&c);
                    }
                }
            } else {
                FAILS.store(0, Ordering::Relaxed);
            }
        },
    );
}

/// Spawns one swaybg per enabled output — mapped image where the
/// per-monitor map has one, the global image otherwise. Falls back
/// to a single swaybg for all outputs when sysfs knows none.
fn spawn_images(
    names: &[String],
    global: &str,
    mode: &str,
    assignments: &std::collections::BTreeMap<String, String>,
) -> Vec<Child> {
    if names.is_empty() {
        return Command::new("swaybg")
            .args(["-i", global, "--mode", mode])
            .spawn()
            .into_iter()
            .collect();
    }
    names
        .iter()
        .filter_map(|n| {
            let img = assignments.get(n).map(String::as_str).unwrap_or(global);
            Command::new("swaybg")
                .args(["-o", n, "-i", img, "--mode", mode])
                .spawn()
                .ok()
        })
        .collect()
}

/// Solid-color background for every output — one process covers all.
fn spawn_color(color: &str) -> Vec<Child> {
    Command::new("swaybg")
        .args(["-m", "solid_color", "-c", color])
        .spawn()
        .into_iter()
        .collect()
}

/// Retires the previous set after the hand-over delay: kill, then
/// wait() them so they are reaped instead of lingering as zombies.
fn retire_children(mut old: Vec<Child>) {
    if old.is_empty() {
        return;
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(HANDOVER_MS), move || {
        for mut c in old.drain(..) {
            let _ = c.kill();
            let _ = c.wait();
        }
    });
}

/// Clones a Choice (kept small enough for a simple manual copy).
fn clone_choice(c: &Choice) -> Choice {
    match c {
        Choice::Image { path, mode } => Choice::Image {
            path: path.clone(),
            mode,
        },
        Choice::SolidColor { color } => Choice::SolidColor {
            color: color.clone(),
        },
    }
}
