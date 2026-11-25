// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// gtk_ini — mirror org.gnome.desktop.interface theme keys into
// ~/.config/gtk-3.0/settings.ini so GTK3 applications pick up
// theme, icon and cursor changes made via GSettings.

use gio::prelude::*;

const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const SECTION: &str = "Settings";

/// True when the interface schema is installed. `Settings::new`
/// aborts on a missing schema, so availability must be probed
/// through the schema source first.
pub fn available() -> bool {
    gio::SettingsSchemaSource::default()
        .map(|s| s.lookup(INTERFACE_SCHEMA, true).is_some())
        .unwrap_or(false)
}

fn interface_settings() -> Option<gio::Settings> {
    available().then(|| gio::Settings::new(INTERFACE_SCHEMA))
}

/// Writes the four mirrored keys into settings.ini, preserving any
/// other entries already present in the file.
pub fn sync() {
    let Some(s) = interface_settings() else { return };

    let config_dir = glib::user_config_dir().join("gtk-3.0");
    let path = config_dir.join("settings.ini");

    let kf = glib::KeyFile::new();
    if path.exists() {
        if let Err(e) = kf.load_from_file(&path, glib::KeyFileFlags::NONE) {
            eprintln!("ragnar-gtk3-sync: failed to load {}: {e}", path.display());
            return;
        }
    }

    kf.set_string(SECTION, "gtk-theme-name", &s.string("gtk-theme"));
    kf.set_string(SECTION, "gtk-icon-theme-name", &s.string("icon-theme"));
    kf.set_string(SECTION, "gtk-cursor-theme-name", &s.string("cursor-theme"));
    kf.set_string(
        SECTION,
        "gtk-cursor-theme-size",
        &s.int("cursor-size").to_string(),
    );

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        eprintln!("ragnar-gtk3-sync: cannot create {}: {e}", config_dir.display());
        return;
    }
    if let Err(e) = kf.save_to_file(&path) {
        eprintln!("ragnar-gtk3-sync: failed to save {}: {e}", path.display());
    }
}

/// Re-syncs on any relevant interface key change. The debounce
/// collapses bursts — a settings panel writes several keys at once
/// and each would otherwise trigger a full file rewrite. The
/// one-shot timer clears its own id when firing so the next change
/// cannot remove() an already-spent source.
pub fn watch() {
    let Some(s) = interface_settings() else { return };
    let pending = std::rc::Rc::new(std::cell::RefCell::new(None::<glib::SourceId>));
    let pending2 = pending.clone();
    s.connect_changed(None, move |_, key| {
        if !matches!(key, "gtk-theme" | "icon-theme" | "cursor-theme" | "cursor-size") {
            return;
        }
        if let Some(id) = pending2.borrow_mut().take() {
            id.remove();
        }
        let pending3 = pending2.clone();
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            pending3.borrow_mut().take();
            sync();
        });
        *pending2.borrow_mut() = Some(id);
    });
}
