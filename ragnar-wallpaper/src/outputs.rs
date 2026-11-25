// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-wallpaper — enabled output connectors via sysfs DRM
//
// Connector names in /sys/class/drm are the kernel names swaybg
// also understands (DP-3, HDMI-A-1, eDP-1, …). No D-Bus round-trip
// to the display service needed for naming outputs.

use std::fs;

const DRM: &str = "/sys/class/drm";

/// Enabled connector names (status == "connected"), sorted for
/// deterministic spawn order.
pub fn enabled() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(DRM)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.file_name().to_string_lossy().into_owned();
            // Entries look like card0-DP-3 — keep only connectors.
            let name = dir.split_once('-').map(|(_, n)| n.to_string())?;
            let status = fs::read_to_string(e.path().join("status")).ok()?;
            (status.trim() == "connected").then_some(name)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}
