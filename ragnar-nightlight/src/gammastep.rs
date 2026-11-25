// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-nightlight — the gammastep child process
//
// Start/stop handling identical to the legacy service: gammastep runs
// detached with `-o -P -c <config>`; stopping sends SIGHUP via
// killall — gammastep exits gracefully and restores the gamma ramp,
// a hard kill would leave the screen tinted.

use crate::config;
use std::ffi::OsStr;

/// Spawns gammastep one-shot-style (applies, keeps running for
/// fades + schedule tracking). Failure is logged, never fatal.
pub fn run() {
    let cfg = config::local_config();
    let argv = [
        OsStr::new("gammastep"),
        OsStr::new("-o"),
        OsStr::new("-P"),
        OsStr::new("-c"),
        cfg.as_os_str(),
    ];
    if let Err(e) = gio::Subprocess::newv(&argv, gio::SubprocessFlags::NONE) {
        eprintln!("ragnar-nightlight: failed to start gammastep: {e}");
    }
}

/// Stops gammastep via SIGHUP — graceful exit, restores gamma.
/// Waits for killall and gives gammastep a moment to release the
/// gamma-control interface, because the next spawn races it.
pub fn stop() {
    let argv = [
        OsStr::new("killall"),
        OsStr::new("-s"),
        OsStr::new("SIGHUP"),
        OsStr::new("gammastep"),
    ];
    if let Ok(p) = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDERR_SILENCE | gio::SubprocessFlags::STDOUT_SILENCE,
    ) {
        let _ = p.wait(gio::Cancellable::NONE);
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
}
