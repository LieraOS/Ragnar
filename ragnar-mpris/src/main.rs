// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-mpris — media-key to MPRIS forwarding daemon
//
// Rust replacement for the MprisController part of the legacy
// ragnar-daemon: owns org.lieraos.ragnar.MprisController and
// forwards media-key methods (from compositor keybinds, raven or
// the taskbar) to whichever MPRIS player is currently active.

mod svc;
mod tracker;

use gio::prelude::*;

fn main() {
    // Bus connection first: tracker seeds from it, the service
    // registers once the name is ours.
    let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        eprintln!("ragnar-mpris: no session bus");
        std::process::exit(1);
    };
    tracker::init(&conn);
    let _owner = svc::own_name();
    glib::MainLoop::new(None, false).run();
}
