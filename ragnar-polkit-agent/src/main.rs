// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-polkit-agent — polkit authentication agent for Ragnar
//
// Session service (autostart, X-GNOME-Autostart-Phase=Desktop):
// runs permanently without a window, exports the
// AuthenticationAgent object on the system bus and only opens its
// GTK4 password dialog when polkitd calls BeginAuthentication.

mod dbus;
mod dialog;
mod helper;
mod i18n;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> glib::ExitCode {
    i18n::init();
    // The id MUST match the session's RequiredComponent name
    // (org.lieraos.ragnar.Polkit = the .desktop basename) — the
    // session manager waits for this bus name before continuing;
    // any other id means a ~90 s component timeout at login.
    let app = adw::Application::builder()
        .application_id("org.lieraos.ragnar.Polkit")
        .build();

    app.connect_activate(|app| {
        // Register on the system bus once; the agent holds no
        // window of its own until polkitd asks for authentication.
        dbus::register(app.clone());
        // The returned guard must live for the whole process —
        // dropping it releases the hold and the agent would exit
        // while idle between authentication requests.
        std::mem::forget(app.hold());
    });

    app.run()
}
