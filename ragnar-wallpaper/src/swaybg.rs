// SPDX-License-Identifier: GPL-3.0-or-later
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
/// Watchdog interval: re-applies when no tracked swaybg is alive.
const WATCHDOG_S: u32 = 15;
/// Consecutive failed spawn checks — caps the retry so a permanently
/// failing swaybg does not loop (the watchdog still retries slowly).
static FAILS: AtomicU32 = AtomicU32::new(0);

/// What to do with the last choice after the watchdog finds all
/// swaybg dead.
pub fn watch_last() {
    glib::timeout_add_local(
        std::time::Duration::from_secs(WATCHDOG_S as u64),
        move || {
            let alive = CURRENT
                .lock()
                .unwrap()
                .iter_mut()
                .any(|c| matches!(c.try_wait(), Ok(None)));
            if !alive {
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
        accounts::set_background(path);
    }
}

/// Verifies the freshly spawned children after the grace window;
/// if any died already (compositor race), retries the whole apply
/// once — a single retry covers the session-start race where the
/// outputs are not yet advertised.
fn check_spawned() {
    glib::timeout_add_local_once(
        std::time::Duration::from_millis(SPAWN_CHECK_MS),
        move || {
            let any_dead = CURRENT
                .lock()
                .unwrap()
                .iter_mut()
                .any(|c| !matches!(c.try_wait(), Ok(None)));
            if any_dead {
                // One automatic retry; after that the 15s watchdog
                // keeps applying — no tight respawn loop.
                if FAILS.fetch_add(1, Ordering::Relaxed) < 1 {
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
