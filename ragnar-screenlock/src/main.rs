// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — screen lock and idle orchestration daemon
//
// Rust replacement for the screenlock part of the legacy ragnar-daemon.
// Owns org.lieraos.ragnar.Screenlock on the session bus and answers:
//   Lock()               spawn the locker (gtklock, swaylock fallback)
//   Dim()/Undim()        idle brightness handling via sysfs + logind
//   ScreenPowerSuspend() / ScreenPowerResume()
//   OnBattery()          AC/battery state via /sys/class/power_supply
//
// It also rebuilds the swayidle command line from the session/power
// GSettings (idle-delay, lock-delay, suspend timers, idle-dim) and
// respawns swayidle whenever those keys change — the same mechanism
// the legacy daemon used, without touching locker internals.

mod backlight;
mod battery;
mod dbus_svc;
mod idle;
mod locker;
mod settings;

use gio::prelude::*;

fn main() {
    // Own the bus name first (REPLACE lets us take it from the legacy
    // daemon during the transition); then rebuild swayidle once and
    // stay resident on the GLib main loop.
    let _owner = dbus_svc::own_name();
    if !idle::rebuild() {
        eprintln!(
            "ragnar-screenlock: required helper binaries missing, idle orchestration disabled"
        );
    }
    glib::MainLoop::new(None, false).run();
}
