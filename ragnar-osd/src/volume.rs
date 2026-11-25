// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-osd — the volume watcher
//
// Replaces the legacy OSDKeys' Gvc.MixerControl watching: a long-lived
// `pactl subscribe` child delivers sink-change events (works against
// PipeWire's pulse-compat layer); on each event the default sink's
// volume + mute are queried and the OSD shows only when the state
// actually changed — no flash on unrelated sink events, no OSD at
// startup (subscribe reports changes only, unlike the mixer READY
// state which the legacy code had to guard against).

use crate::window;
use gio::prelude::*;
use std::cell::RefCell;

thread_local! {
    /// Last displayed state — dedupe so unrelated sink events don't
    /// flash the OSD.
    static LAST: RefCell<Option<(i64, bool)>> = const { RefCell::new(None) };
}

/// Spawns `pactl subscribe` and hooks stdout line parsing into the
/// glib main loop.
pub fn watch() {
    // stdbuf -oL forces line-buffered stdout — pactl's events are
    // stdio-buffered on pipes and would otherwise only arrive in
    // 4 KiB batches instead of per event.
    let argv = [
        std::ffi::OsStr::new("stdbuf"),
        std::ffi::OsStr::new("-oL"),
        std::ffi::OsStr::new("pactl"),
        std::ffi::OsStr::new("subscribe"),
    ];
    let Ok(child) = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    ) else {
        eprintln!("ragnar-osd: cannot spawn pactl subscribe");
        return;
    };
    let Some(stdout) = child.stdout_pipe() else {
        return;
    };
    let stream = gio::DataInputStream::new(&stdout);
    read_line(stream);
}

/// Reads one line asynchronously and re-arms itself — cheap event
/// pump without a dedicated thread.
fn read_line(stream: gio::DataInputStream) {
    let stream2 = stream.clone();
    stream.read_line_async(
        glib::Priority::DEFAULT,
        gio::Cancellable::NONE,
        move |res| {
            if let Ok(line) = res {
                if !line.is_empty() {
                    let text = String::from_utf8_lossy(&line);
                    if text.contains("sink") && text.contains("change") {
                        query_and_show();
                    }
                }
            }
            read_line(stream2.clone());
        },
    );
}

/// Queries default-sink volume and mute, then shows the OSD when the
/// state differs from the last shown one.
fn query_and_show() {
    let Some(vol) = run(&["get-sink-volume", "@DEFAULT_SINK@"]) else {
        return;
    };
    let Some(mute) = run(&["get-sink-mute", "@DEFAULT_SINK@"]) else {
        return;
    };
    // "Volume: front-left: 65536 / 100% / 0.00 dB, ..." → first % wins
    let percent = vol
        .split_whitespace()
        .find(|w| w.ends_with('%'))
        .and_then(|w| w.trim_end_matches('%').parse::<i64>().ok());
    let muted = mute.contains("yes");
    let Some(percent) = percent else {
        return;
    };
    let state = (percent, muted);
    // Show only on a real change — dedupe against the last state.
    // The FIRST event only seeds LAST without showing: PipeWire
    // emits sink-change noise at session start (applying the saved
    // volume counts as a change) and the OSD must not pop up then.
    let changed = LAST.with(|l| {
        let mut l = l.borrow_mut();
        if *l == Some(state) {
            false
        } else {
            let first = l.is_none();
            *l = Some(state);
            !first
        }
    });
    if !changed {
        return;
    }
    window::show(
        Some(icon_for(percent, muted)),
        None,
        Some(percent as f64 / 100.0),
    );
}

/// Runs a pactl subcommand and returns trimmed stdout.
fn run(args: &[&str]) -> Option<String> {
    let mut cmd: Vec<&std::ffi::OsStr> = vec![std::ffi::OsStr::new("pactl")];
    cmd.extend(args.iter().map(std::ffi::OsStr::new));
    let (stdout, _) = gio::Subprocess::newv(
        &cmd,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .ok()?
    .communicate_utf8(None, gio::Cancellable::NONE)
    .ok()?;
    stdout.map(|s| s.trim().to_string())
}

/// Icon name for the current level — same tier table the legacy code
/// carried over from gnome-shell's volume.js.
fn icon_for(percent: i64, muted: bool) -> &'static str {
    if muted || percent == 0 {
        "audio-volume-muted-symbolic"
    } else if percent <= 33 {
        "audio-volume-low-symbolic"
    } else if percent <= 66 {
        "audio-volume-medium-symbolic"
    } else if percent <= 100 {
        "audio-volume-high-symbolic"
    } else {
        "audio-volume-overamplified-symbolic"
    }
}
