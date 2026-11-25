// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-osd — on-screen display daemon
//
// Rust replacement for the OSD part of the legacy ragnar-daemon:
// owns org.lieraos.ragnar.OSD, renders the layer-shell overlay for
// Show() calls, and includes the built-in volume/brightness watchers
// (the former OSDKeys) so volume and brightness keys show the OSD
// end-to-end.

mod brightness;
mod config;
mod svc;
mod volume;
mod window;

use gtk4::prelude::*;

fn main() {
    // GTK init is the only hard requirement — the window, the bus
    // name and the watchers all live on the main loop.
    let app = gtk4::Application::builder()
        .application_id("org.lieraos.ragnar.OSDApp")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(|app| {
        config::load();
        config::watch();
        window::init();
        let _owner = svc::own_name();
        volume::watch();
        brightness::watch();
        // The OSD window stays hidden until an event arrives — without
        // a hold GtkApplication would quit right after activate because
        // no toplevel is visible. The guard must live for the whole
        // process lifetime, so it is deliberately leaked here.
        std::mem::forget(app.hold());
    });
    app.run();
}
