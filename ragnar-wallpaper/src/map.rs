// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-wallpaper — per-output wallpaper map
//
// ragnar-settings writes connector=image assignments to
// ~/.config/ragnar-settings/wallpaper-outputs. This service owns
// applying them: mapped outputs get their image, everything else
// falls back to the global picture-uri.

use gio::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Path of the assignment file written by ragnar-settings.
pub fn path() -> PathBuf {
    let mut p = glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("wallpaper-outputs");
    p
}

/// Loads connector → image-path assignments.
pub fn load() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(s) = std::fs::read_to_string(path()) {
        for line in s.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let (k, v) = (k.trim(), v.trim());
                if !k.is_empty() && !v.is_empty() {
                    map.insert(k.to_string(), v.to_string());
                }
            }
        }
    }
    map
}

/// Re-runs the callback whenever the map file changes — Settings
/// writes it on every per-monitor apply, which is exactly our cue
/// to respawn swaybg.
pub fn watch<F: Fn() + 'static>(cb: F) {
    let file = gio::File::for_path(path());
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir.path().unwrap_or_default());
    }
    if let Ok(monitor) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        // Keep the monitor alive for the process lifetime.
        std::mem::forget(monitor.clone());
        monitor.connect_changed(move |_, _, _, ev| {
            if ev != gio::FileMonitorEvent::Deleted {
                cb();
            }
        });
    }
}
