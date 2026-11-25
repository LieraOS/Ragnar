// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-wallpaper — compositor readiness gate
//
// At session start the autostart unit can race the compositor: the
// Wayland socket does not exist yet, or a stale socket file from the
// previous session still sits in XDG_RUNTIME_DIR and refuses
// connections. Spawning swaybg into that window kills it instantly.
// This gate polls the display socket until a connection is accepted
// — the compositor is then actually listening — before the first
// apply runs.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Maximum wait before giving up and applying anyway — keeps the
/// service from deadlocking when no compositor is ever coming (the
/// retry watchdog still heals later if one appears).
const TIMEOUT: Duration = Duration::from_secs(30);
/// Poll interval between connection attempts.
const POLL: Duration = Duration::from_millis(100);

/// Path of the session Wayland socket: an absolute WAYLAND_DISPLAY
/// is used as-is, otherwise it is resolved inside XDG_RUNTIME_DIR
/// (defaulting to "wayland-0", matching libwayland's own fallback).
fn socket_path() -> Option<PathBuf> {
    let name = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string());
    if name.starts_with('/') {
        return Some(PathBuf::from(name));
    }
    let dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    Some(PathBuf::from(dir).join(name))
}

/// Blocks until the Wayland socket accepts a connection or the
/// timeout expires. Returns true when a live compositor was seen.
pub fn wait_for_compositor() -> bool {
    let Some(sock) = socket_path() else {
        return false;
    };
    let deadline = Instant::now() + TIMEOUT;
    loop {
        // A successful connect means a compositor is listening; the
        // probe connection is dropped immediately afterwards.
        if UnixStream::connect(&sock).is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL);
    }
}
