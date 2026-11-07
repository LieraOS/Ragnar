// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — AMD/Intel GPU information via sysfs hwmon
// Copyright (C) 2026 LieraOS Project
//
// amdgpu and i915 expose temperature, power and fan readings through
// hwmon under /sys/class/drm/cardN/device/hwmon/hwmonM/. Values are in
// milli-units (millidegrees, microwatts) or RPM.

use std::fs;
use std::path::Path;

use ragnar_shared::gpu::{GpuInfo, GpuVendor};

/// Query one AMD or Intel GPU from sysfs. Returns a GpuInfo with
/// whatever the hwmon interface exposes; missing files yield -1/0.
pub fn query(
    card_index: i32,
    vendor: GpuVendor,
    pci_address: &str,
    driver: &str,
    device: &Path,
) -> Option<GpuInfo> {
    let hwmon = find_hwmon(device);

    let temp_mc = hwmon.as_ref().and_then(|h| read_i64(h.join("temp1_input")));
    let power_uw = hwmon
        .as_ref()
        .and_then(|h| read_i64(h.join("power1_average")))
        .or_else(|| hwmon.as_ref().and_then(|h| read_i64(h.join("power1_input"))));
    let cap_uw = hwmon.as_ref().and_then(|h| read_i64(h.join("power1_cap")));
    let cap_max = hwmon
        .as_ref()
        .and_then(|h| read_i64(h.join("power1_cap_max")));
    let cap_min = hwmon
        .as_ref()
        .and_then(|h| read_i64(h.join("power1_cap_min")));
    let fan_rpm = hwmon.as_ref().and_then(|h| read_i64(h.join("fan1_input")));
    let pwm = hwmon.as_ref().and_then(|h| read_i64(h.join("pwm1")));

    // Fan speed: amdgpu exposes pwm1 (0-255) alongside fan1_input (rpm).
    let fan_percent = match pwm {
        Some(p) => ((p * 100) / 255) as i32,
        None => {
            if fan_rpm.is_some() {
                // RPM without a known max — report 0 rather than guessing.
                0
            } else {
                -1
            }
        }
    };

    // amdgpu power1_cap is writable by root — adjustable when present.
    let power_writable = cap_uw.is_some() && cap_max.unwrap_or(0) > cap_min.unwrap_or(0);
    let fan_writable = hwmon
        .as_ref()
        .map(|h| h.join("pwm1").exists() && h.join("pwm1_enable").exists())
        .unwrap_or(false);

    Some(GpuInfo {
        card_index,
        vendor,
        name: device_name(device, vendor),
        driver: driver.to_string(),
        pci_address: pci_address.to_string(),
        temperature_c: temp_mc.map(|t| (t / 1000) as i32).unwrap_or(-1),
        power_draw_w: power_uw.map(|p| p as f64 / 1_000_000.0).unwrap_or(0.0),
        power_limit_w: cap_uw.map(|p| p as f64 / 1_000_000.0).unwrap_or(0.0),
        power_limit_min_w: cap_min.map(|p| p as f64 / 1_000_000.0).unwrap_or(0.0),
        power_limit_max_w: cap_max.map(|p| p as f64 / 1_000_000.0).unwrap_or(0.0),
        fan_percent,
        core_clock_mhz: read_clock(device, "pp_dpm_sclk"),
        mem_clock_mhz: read_clock(device, "pp_dpm_mclk"),
        max_graphics_clock_mhz: 0,
        supports_power_limit: power_writable,
        // Fan curves and clock offsets need root writes to hwmon /
        // pp_od_clk_voltage; the capability is reported, privilege is
        // requested per-write through pkexec in the service layer.
        supports_fan_control: fan_writable,
        supports_overclock: device.join("pp_od_clk_voltage").exists(),
    })
}

/// Find the hwmon directory below a card device node.
fn find_hwmon(device: &Path) -> Option<std::path::PathBuf> {
    let hwmon_root = device.join("hwmon");
    fs::read_dir(hwmon_root).ok()?.flatten().next().map(|e| e.path())
}

/// Resolve the hwmon directory for a PCI address (e.g. "0000:01:00.0").
/// Used by the write paths to locate power1_cap/pwm1 for a GPU.
pub fn hwmon_dir_for_pci(pci_address: &str) -> Option<std::path::PathBuf> {
    let hwmon_root = Path::new("/sys/bus/pci/devices")
        .join(pci_address)
        .join("hwmon");
    fs::read_dir(hwmon_root).ok()?.flatten().next().map(|e| e.path())
}

/// Read an integer sysfs file, trimming whitespace.
fn read_i64(path: std::path::PathBuf) -> Option<i64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Read the active clock from a pp_dpm_* file ("*: 1200Mhz *" line).
fn read_clock(device: &Path, name: &str) -> i32 {
    let text = match fs::read_to_string(device.join(name)) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    for line in text.lines() {
        if line.ends_with('*') {
            return line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.trim_end_matches("Mhz").parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Resolve a display name: prefer the kernel device name, else vendor.
fn device_name(device: &Path, vendor: GpuVendor) -> String {
    if let Ok(name) = fs::read_to_string(device.join("product_name")) {
        let name = name.trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    match vendor {
        GpuVendor::Amd => "AMD GPU".to_string(),
        GpuVendor::Intel => "Intel GPU".to_string(),
        GpuVendor::Nvidia => "NVIDIA GPU".to_string(),
        GpuVendor::Unknown => "GPU".to_string(),
    }
}
