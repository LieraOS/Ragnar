// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — animated-wallpaper flag
// Copyright (C) 2026 LieraOS Project
//
// The toggle lives in ~/.config/ragnar-settings/wallpaper-animated
// as a plain "true"/"false" line — the same file mechanism as the
// per-monitor map. ragnar-wallpaper watches it and swaps swaybg
// for ragnar-livewall (or back) on every write.

use std::path::PathBuf;

/// Flag file path, sibling of the per-monitor map.
fn flag_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("wallpaper-animated");
    p
}

/// Whether the animated wallpaper is enabled.
pub fn enabled() -> bool {
    std::fs::read_to_string(flag_path())
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// Persist the toggle — ragnar-wallpaper's file monitor picks it
/// up immediately and re-applies the wallpaper layer.
pub fn set_enabled(on: bool) {
    let p = flag_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, if on { "true\n" } else { "false\n" });
}
