// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — GSettings integration
// Copyright (C) 2026 LieraOS Project
//
// Writes the primary monitor to the Ragnar panel's GSettings key.
// The panel watches `org.lieraos.ragnar.panel primary-monitor-list`
// and repositions the taskbar live when the value changes.

use std::process::Command;

/// Update the panel's primary-monitor-list GSettings key.
/// `connector` is the output connector name (e.g. "DP-3").
pub fn write_primary_monitor(connector: &str) {
    let value = format!("['{}']", connector);
    let result = Command::new("gsettings")
        .args([
            "set",
            "org.lieraos.ragnar.panel",
            "primary-monitor-list",
            &value,
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            eprintln!(
                "ragnar-display-svc: gsettings primary-monitor-list -> {}",
                connector
            );
        }
        Ok(out) => {
            eprintln!(
                "ragnar-display-svc: gsettings failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Err(e) => {
            eprintln!("ragnar-display-svc: gsettings spawn failed: {}", e);
        }
    }
}
