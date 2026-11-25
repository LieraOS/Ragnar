// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenshot — modern GTK4/adw screenshot tool for LieraOS
// Ragnar (labwc/Wayland, wlr-screencopy backend).
mod editor;
mod i18n;
mod model;
mod overlay;
mod toplevel;
mod ui;
mod wayland;

use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static UI: RefCell<Option<Rc<ui::AppUi>>> = const { RefCell::new(None) };
}

fn main() {
    // Diagnostic path: RAGNAR_SHOT_PROBE=1 captures all monitors
    // headlessly, prints the advertised shm format and writes a
    // PPM for pixel-level comparison against grim.
    if std::env::var_os("RAGNAR_SHOT_PROBE").is_some() {
        let mut wl = wayland::Wayland::new().expect("wayland connect");
        let img = wl.capture_all(false).expect("probe capture");
        let mut out = Vec::with_capacity(img.width * img.height * 3);
        for px in img.pixels.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0]]); // BGRA -> RGB
        }
        let hdr = format!("P6\n{} {}\n255\n", img.width, img.height);
        std::fs::write("/tmp/ragnar-probe.ppm", [hdr.into_bytes(), out].concat()).unwrap();
        println!("wrote /tmp/ragnar-probe.ppm {}x{}", img.width, img.height);
        return;
    }
    i18n::init();
    let app = adw::Application::builder()
        .application_id("org.lieraos.ragnar.Screenshot")
        .build();
    app.connect_activate(|app| {
        let _ = adw::init();
        let ui = ui::AppUi::new(app);
        // Keep a strong reference for the app lifetime — all button
        // handlers hold only weak clones and would be dead without it.
        UI.with(|slot| *slot.borrow_mut() = Some(ui.clone()));
        ui.window.present();
    });
    app.run();
}
