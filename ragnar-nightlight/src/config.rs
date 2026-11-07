// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-nightlight — GSettings → gammastep config translation
//
// Watches org.gnome.settings-daemon.plugins.color (the standard gsd
// color schema Ragnar Settings writes) and rewrites
// ~/.config/gammastep/ragnar_config.ini — ported 1:1 from the legacy
// NightLightManager so existing settings keep working unchanged.

use gio::prelude::*;
use std::path::PathBuf;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.color";
const LOCAL_NAME: &str = "ragnar_config.ini";
const SEARCH_NAME: &str = "gammastep.config";

/// Where gammastep's working config lives for this user.
pub fn local_config() -> PathBuf {
    let mut p = glib::user_config_dir();
    p.push("gammastep");
    p.push(LOCAL_NAME);
    p
}

/// Watches the color schema and runs `apply` on every change.
pub fn watch(settings: &gio::Settings, apply: impl Fn(&gio::Settings) + 'static) {
    settings.connect_changed(None, move |s, _| apply(s));
}

/// Whether night light is currently enabled.
pub fn enabled(settings: &gio::Settings) -> bool {
    settings.boolean("night-light-enabled")
}

/// The gio Settings object for the color schema, or None when the
/// schema is missing (never fatal — daemon keeps running).
pub fn settings() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source
        .lookup(SCHEMA, true)
        .map(|_| gio::Settings::new(SCHEMA))
}

/// Makes sure the local config file exists — seeds it from the first
/// match along the search path (user file, then the distro/system
/// shipped variants), exactly like the legacy version.
pub fn ensure_local() {
    let local = local_config();
    let mut candidates = vec![local.clone()];
    for dir in glib::system_data_dirs() {
        candidates.push(
            dir.join("ragnar-desktop")
                .join(format!("distro-{SEARCH_NAME}")),
        );
        candidates.push(dir.join("ragnar-desktop").join(SEARCH_NAME));
    }
    let Some(found) = candidates.iter().find(|p| p.exists()) else {
        eprintln!("ragnar-nightlight: no {SEARCH_NAME} found anywhere");
        return;
    };
    if *found == local || local.exists() {
        return;
    }
    if let Some(d) = local.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::copy(found, &local);
}

/// Rewrites the local ini from current GSettings — temp, provider
/// and manual dusk/dawn times.
pub fn update(settings: &gio::Settings) {
    ensure_local();
    let path = local_config();
    let mut kf = glib::KeyFile::new();
    if kf.load_from_file(&path, glib::KeyFileFlags::NONE).is_err() {
        return;
    }
    let temp = settings.uint("night-light-temperature") as u64;
    kf.set_uint64("general", "temp-night", temp);
    let provider = if settings.boolean("night-light-schedule-automatic") {
        "geoclue2"
    } else {
        "manual"
    };
    kf.set_string("general", "location-provider", provider);
    let dusk = settings.double("night-light-schedule-from");
    let dawn = settings.double("night-light-schedule-to");
    if (dusk - dawn).abs() < f64::EPSILON {
        // "All the time": equal day/night temperatures — warm around
        // the clock, no dawn/dusk timing needed. gammastep rejects a
        // 0:00/0:00 pair as invalid.
        kf.set_uint64("general", "temp-day", temp);
        kf.set_string("manual", "lat", "0.0");
        kf.set_string("manual", "lon", "0.0");
    } else {
        kf.set_string("general", "dusk-time", &format!("{}:00", dusk as i32));
        kf.set_string("general", "dawn-time", &format!("{}:00", dawn as i32));
    }
    let _ = kf.save_to_file(&path);
}
