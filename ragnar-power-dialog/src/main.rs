// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — session action chooser for the Ragnar desktop
//
// Started on demand by the taskbar start-menu power button (or any
// keybind/menu entry): shows the session actions the user enabled
// in Ragnar Settings and that the machine can perform, executes the
// chosen one and exits.

mod actions;
mod caps;
mod config;
mod daemon;
mod dbus_shim;
mod i18n;
mod session;
mod window;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> glib::ExitCode {
    i18n::init();
    // NOTE: must NOT be org.lieraos.ragnar.PowerDialog — the legacy
    // legacy session daemon already owns that bus name; claiming it
    // made this process a remote instance that forwarded activate
    // into the void and exited silently. The names above are owned
    // explicitly in --daemon mode, not via the application id.
    let app = adw::Application::builder()
        .application_id("org.lieraos.ragnar.PowerMenu")
        .build();
    app.connect_activate(window::present);
    if std::env::args().any(|a| a == "--daemon") {
        return daemon::run(&app);
    }
    app.run()
}
