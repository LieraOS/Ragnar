// SPDX-License-Identifier: GPL-3.0-or-later
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
