// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — AC/battery state via sysfs
//
// UPower's get_on_battery() equals: a battery exists and no AC
// adapter is online. Reading /sys/class/power_supply directly keeps
// the service dependency-free (no libupower binding needed).

use std::fs;
use std::path::Path;

const SYSFS: &str = "/sys/class/power_supply";

/// True when running on battery (battery present, no mains online).
/// Desktops without any battery report false — same as UPower.
pub fn on_battery() -> bool {
    let Ok(entries) = fs::read_dir(SYSFS) else {
        return false;
    };
    let mut has_battery = false;
    for e in entries.flatten() {
        let dir = e.path();
        match fs::read_to_string(dir.join("type"))
            .map(|s| s.trim().to_string())
            .as_deref()
        {
            Ok("Mains") => {
                if fs::read_to_string(dir.join("online"))
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false)
                {
                    return false; // a mains adapter is online
                }
            }
            Ok("Battery") => has_battery = true,
            _ => {}
        }
    }
    // Battery present and no mains reported online → on battery.
    // Without mains entries at all (rare) fall back to battery-only.
    has_battery
        && !Path::new(SYSFS)
            .read_dir()
            .map(|mut d| {
                d.any(|e| {
                    e.ok()
                        .and_then(|e| {
                            fs::read_to_string(e.path().join("type"))
                                .ok()
                                .map(|s| {
                                    s.trim() == "Mains"
                                        && fs::read_to_string(e.path().join("online"))
                                            .map(|o| o.trim() == "1")
                                            .unwrap_or(false)
                                })
                                .map(|on| on)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
}
