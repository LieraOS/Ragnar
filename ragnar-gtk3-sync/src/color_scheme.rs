// SPDX-License-Identifier: GPL-3.0-or-later
// color_scheme — keep org.lieraos.ragnar.panel dark-theme and the
// freedesktop org.gnome.desktop.interface color-scheme consistent.
// ragnar-settings already writes both keys together; this watcher
// only matters when another client changes dark-theme alone.

use gio::prelude::*;

const PANEL_SCHEMA: &str = "org.lieraos.ragnar.panel";
const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";

fn schema_installed(id: &str) -> bool {
    gio::SettingsSchemaSource::default()
        .map(|s| s.lookup(id, true).is_some())
        .unwrap_or(false)
}

/// Applies dark-theme → color-scheme once. Writes only on actual
/// divergence so a watcher can never echo into itself.
pub fn sync() {
    if !schema_installed(PANEL_SCHEMA) || !schema_installed(INTERFACE_SCHEMA) {
        return;
    }
    let panel = gio::Settings::new(PANEL_SCHEMA);
    let iface = gio::Settings::new(INTERFACE_SCHEMA);

    let want = if panel.boolean("dark-theme") {
        "prefer-dark"
    } else {
        "prefer-light"
    };
    if iface.string("color-scheme") != want {
        let _ = iface.set_string("color-scheme", want);
    }
}

/// Re-applies on every dark-theme change.
pub fn watch() {
    if !schema_installed(PANEL_SCHEMA) {
        return;
    }
    let panel = gio::Settings::new(PANEL_SCHEMA);
    panel.connect_changed(Some("dark-theme"), move |_, _| sync());
}
