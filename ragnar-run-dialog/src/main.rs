// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-run-dialog — launcher overlay for the Ragnar desktop
//
// Started on demand by the compositor keybind (Alt+F2): opens a
// krunner-style search box anchored to the top of the focused
// monitor, lists matching applications, runs the picked entry or
// the typed text as a command, and exits.

mod apps;
mod css;
mod i18n;
mod launch;
mod model;
mod window;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> glib::ExitCode {
    i18n::init();
    let app = adw::Application::builder()
        .application_id("org.lieraos.ragnar.RunDialog")
        .build();
    app.connect_activate(window::present);
    app.run()
}
