// SPDX-License-Identifier: GPL-3.0-or-later
//! ragnar-software — updates and package management for Ragnar.
//!
//! v0.0.1 scope: welcome page with background update status plus an
//! updates page listing pending updates for selective install.
//! Backend: PackageKit over the system D-Bus (polkit, transactions
//! and progress are handled by packagekitd).

mod backend;
mod catalog;
mod catalog_data;
mod history;
mod i18n;
mod packagekit;
mod settings;
mod updates;
mod welcome;
mod window;

use gtk4::prelude::*;
use libadwaita as adw;

fn main() {
    i18n::init();
    let app = adw::Application::builder()
        .application_id("org.lieraos.ragnar.Software")
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    // File arguments (.deb/.flatpakref) arrive via `open`, not argv.
    app.connect_open(|app, files, _hint| {
        if let Some(p) = files.first().and_then(|f| f.path()) {
            window::build_install(app, p.to_string_lossy().to_string());
        }
    });
    app.connect_activate(window::build);
    app.run();
}
