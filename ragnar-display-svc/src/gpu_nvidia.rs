// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — NVIDIA GPU information via nvidia-smi
// Copyright (C) 2026 LieraOS Project
//
// nvidia-smi exposes temperature, power, fan and clock data to
// unprivileged users. A "N/A" field means the GPU/driver does not
// support that metric and is mapped to -1 or 0.

use std::process::Command;

use ragnar_shared::gpu::{GpuInfo, GpuVendor};

/// Query one NVIDIA GPU by PCI bus address via nvidia-smi.
/// Returns None when the GPU is not listed by nvidia-smi.
pub fn query(card_index: i32, pci_address: &str, driver: &str) -> Option<GpuInfo> {
    // nvidia-smi wants the long form "00000000:01:00.0".
    let bus_id = if pci_address.len() == 7 {
        format!("00000000:{}", pci_address)
    } else {
        pci_address.to_string()
    };

    let fields = [
        "name",
        "driver_version",
        "temperature.gpu",
        "power.draw",
        "power.limit",
        "power.min_limit",
        "power.max_limit",
        "fan.speed",
        "clocks.gr",
        "clocks.mem",
        "clocks.max.gr",
    ];

    let out = Command::new("nvidia-smi")
        .args([
            "-i",
            &bus_id,
            "--query-gpu",
            &fields.join(","),
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.to_string();
    let mut cols = line.split(',').map(|c| c.trim().to_string());

    let name = cols.next().unwrap_or_default();
    let driver_version = cols.next().unwrap_or_else(|| driver.to_string());
    let temp = parse_i32(&cols.next().unwrap_or_default(), -1);
    let power_draw = parse_f64(&cols.next().unwrap_or_default(), 0.0);
    let power_limit = parse_f64(&cols.next().unwrap_or_default(), 0.0);
    let power_min = parse_f64(&cols.next().unwrap_or_default(), 0.0);
    let power_max = parse_f64(&cols.next().unwrap_or_default(), 0.0);
    let fan = parse_i32(&cols.next().unwrap_or_default(), -1);
    let core = parse_i32(&cols.next().unwrap_or_default(), 0);
    let mem = parse_i32(&cols.next().unwrap_or_default(), 0);
    let max_gr = parse_i32(&cols.next().unwrap_or_default(), 0);

    // Power limit is adjustable when the driver reports a usable range.
    let power_writable = power_min > 0.0 && power_max > power_min;

    Some(GpuInfo {
        card_index,
        vendor: GpuVendor::Nvidia,
        name,
        driver: driver_version,
        pci_address: pci_address.to_string(),
        temperature_c: temp,
        power_draw_w: power_draw,
        power_limit_w: power_limit,
        power_limit_min_w: power_min,
        power_limit_max_w: power_max,
        fan_percent: fan,
        core_clock_mhz: core,
        mem_clock_mhz: mem,
        max_graphics_clock_mhz: max_gr,
        supports_power_limit: power_writable,
        // NVML cannot set fan curves or clock offsets on consumer
        // GeForce cards without the X11 NV-CONTROL extension, which
        // is unavailable on Wayland — report those as unsupported.
        supports_fan_control: false,
        supports_overclock: false,
    })
}

/// Parse an integer field, mapping "N/A" and parse failures to `fallback`.
fn parse_i32(value: &str, fallback: i32) -> i32 {
    if value == "N/A" || value.is_empty() {
        return fallback;
    }
    // Fields like "1569 MHz" may keep a unit when nounits is ignored.
    value
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v as i32)
        .unwrap_or(fallback)
}

/// Parse a float field, mapping "N/A" and parse failures to `fallback`.
fn parse_f64(value: &str, fallback: f64) -> f64 {
    if value == "N/A" || value.is_empty() {
        return fallback;
    }
    value
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(fallback)
}
