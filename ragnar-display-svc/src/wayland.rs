// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — Wayland output detection via wlr-randr
// Copyright (C) 2026 LieraOS Project
//
// Uses wlr-randr to query the compositor for connected outputs.
// wlr-randr speaks wlr-output-management internally, so we get
// the same data without reimplementing the Wayland protocol client.

use std::process::Command;

use ragnar_shared::display::{Head, Mode};

/// Query connected outputs by calling `wlr-randr` and parsing its output.
/// Returns a list of heads with current modes and positions.
pub fn detect_outputs() -> Vec<Head> {
    let output = Command::new("wlr-randr").output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_wlr_randr(&stdout)
        }
        _ => Vec::new(),
    }
}

/// Parse human-readable `wlr-randr` output.
///
/// Format (per output):
///   <connector> "<description>"
///     Make: <manufacturer>
///     Model: <model>
///     Serial: <serial>
///     Physical size: <w>x<h> mm
///     Enabled: yes|no
///     Modes:
///       <w>x<h> px, <hz> Hz (preferred, current)
///       ...
///     Position: <x>, <y>
///     Transform: <transform>
///     Scale: <scale>
///     Adaptive Sync: enabled|disabled
fn parse_wlr_randr(text: &str) -> Vec<Head> {
    let mut heads = Vec::new();
    let mut current: Option<Head> = None;
    let mut current_mode: Option<Mode> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        // A non-indented line starting a new output connector.
        // Format: `DP-3 "Philips Consumer Electronics Company Philips 245P DL41208804582 (DP-3)"`
        // Connector names start with letters/digits and are followed by a space+quote.
        if !line.starts_with(' ') && !trimmed.is_empty() && trimmed.contains('"') {
            // Save previous head.
            if let Some(mut head) = current.take() {
                head.current_mode = current_mode.take().unwrap_or(zero_mode());
                heads.push(head);
            }

            // Parse connector name (everything before the first space).
            let name = trimmed.split_whitespace().next().unwrap_or("").to_string();

            current = Some(Head {
                name,
                model: String::new(),
                manufacturer: String::new(),
                serial: String::new(),
                enabled: false,
                current_mode: zero_mode(),
                modes: Vec::new(),
                position: (0, 0),
                primary: false,
                scale: 1.0,
                rotation: 0,
                adaptive_sync: false,
            });
            current_mode = None;
        }

        let head = match &mut current {
            Some(h) => h,
            None => continue,
        };

        // Parse indented property lines.
        if let Some(value) = trimmed.strip_prefix("Make: ") {
            head.manufacturer = value.to_string();
        } else if let Some(value) = trimmed.strip_prefix("Model: ") {
            head.model = value.to_string();
        } else if let Some(value) = trimmed.strip_prefix("Serial: ") {
            head.serial = value.to_string();
        } else if let Some(value) = trimmed.strip_prefix("Enabled: ") {
            head.enabled = value == "yes";
        } else if let Some(value) = trimmed.strip_prefix("Position: ") {
            let parts: Vec<&str> = value.split(',').collect();
            if parts.len() == 2 {
                let x: i32 = parts[0].trim().parse().unwrap_or(0);
                let y: i32 = parts[1].trim().parse().unwrap_or(0);
                head.position = (x, y);
            }
        } else if let Some(value) = trimmed.strip_prefix("Scale: ") {
            head.scale = value.parse().unwrap_or(1.0);
        } else if let Some(value) = trimmed.strip_prefix("Transform: ") {
            head.rotation = match value {
                "90" => 90,
                "180" => 180,
                "270" => 270,
                _ => 0,
            };
        } else if let Some(value) = trimmed.strip_prefix("Adaptive Sync: ") {
            head.adaptive_sync = value == "enabled";
        } else if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            // Mode line: "1920x1200 px, 59.950001 Hz (preferred, current)"
            if let Some(mode) = parse_mode_line(trimmed) {
                let is_current = trimmed.contains("current");
                head.modes.push(mode.clone());
                if is_current {
                    current_mode = Some(mode);
                }
            }
        }
    }

    // Save last head.
    if let Some(mut head) = current {
        head.current_mode = current_mode.unwrap_or(zero_mode());
        heads.push(head);
    }

    heads
}

/// Parse a mode line: "1920x1200 px, 59.950001 Hz (preferred, current)"
fn parse_mode_line(line: &str) -> Option<Mode> {
    // Split only at the first comma (resolution vs refresh rate).
    // The "(preferred, current)" suffix contains commas we must ignore.
    let comma_pos = line.find(',')?;
    let res_part = line[..comma_pos].trim();
    let rest = line[comma_pos + 1..].trim();

    // Resolution: "1920x1200 px"
    let res_str = res_part.strip_suffix(" px").unwrap_or(res_part);
    let dims: Vec<&str> = res_str.split('x').collect();
    if dims.len() != 2 {
        return None;
    }
    let width: i32 = dims[0].trim().parse().ok()?;
    let height: i32 = dims[1].trim().parse().ok()?;

    // Refresh rate: "59.950001 Hz (preferred, current)"
    // Take only the numeric part before " Hz".
    let hz_str: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let hz: f64 = hz_str.parse().ok()?;
    let refresh_rate = (hz * 1000.0) as i32;

    Some(Mode {
        width,
        height,
        refresh_rate,
        preferred: line.contains("preferred"),
    })
}

/// Create a zero mode (used when no current mode is set).
fn zero_mode() -> Mode {
    Mode {
        width: 0,
        height: 0,
        refresh_rate: 0,
        preferred: false,
    }
}
