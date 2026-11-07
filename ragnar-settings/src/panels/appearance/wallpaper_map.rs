// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — per-monitor wallpaper assignment + swaybg spawn
// Copyright (C) 2026 LieraOS Project
//
// Connector names come from DisplayServices at runtime — nothing is
// hardcoded, so this works on any monitor layout. Assignments are a
// plain key=value map in ~/.config/ragnar-settings/wallpaper-outputs;
// outputs without an entry fall back to the global picture-uri.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::super::display::dbus_client;

/// Map file: one `connector=image-path` line per override.
fn map_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("wallpaper-outputs");
    p
}

/// Load connector → image-path assignments.
pub fn load() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(s) = std::fs::read_to_string(map_path()) {
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

/// Persist the full assignment map.
pub fn save(map: &BTreeMap<String, String>) {
    let p = map_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body: String = map.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    let _ = std::fs::write(p, body);
}

/// Live output connector names from DisplayServices
/// (enabled heads only, e.g. DP-3, HDMI-A-1, eDP-1).
pub fn outputs() -> Vec<String> {
    let mut names: Vec<String> = dbus_client::fetch_outputs()
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.name.clone())
        .collect();
    names.sort();
    names
}
