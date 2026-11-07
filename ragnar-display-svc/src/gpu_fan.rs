// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — GPU fan speed writes
// Copyright (C) 2026 LieraOS Project
//
// AMD exposes manual fan control through hwmon: pwm1_enable=1 switches
// to manual mode, pwm1 (0-255) sets the speed. Writes need root, so the
// command runs via pkexec and the user gets a polkit dialog.

use std::process::Command;

/// Set the fan speed in percent (0-100) for the GPU at `pci_address`.
/// Only implemented for GPUs exposing pwm1 via hwmon (AMD).
pub fn set_fan_percent(pci_address: &str, percent: u8) -> Result<(), String> {
    let hwmon = crate::gpu_sysfs::hwmon_dir_for_pci(pci_address)
        .ok_or_else(|| "hwmon not found for this GPU".to_string())?;

    let pwm = hwmon.join("pwm1");
    let pwm_enable = hwmon.join("pwm1_enable");
    if !pwm.exists() || !pwm_enable.exists() {
        return Err("fan control not supported by this GPU".to_string());
    }

    let pwm_value = (percent.min(100) as u32 * 255) / 100;
    let script = format!(
        "echo 1 > {} && echo {} > {}",
        pwm_enable.display(),
        pwm_value,
        pwm.display()
    );

    let out = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("pkexec spawn failed: {}", e))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
