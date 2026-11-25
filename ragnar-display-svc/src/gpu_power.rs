// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — GPU power limit writes
// Copyright (C) 2026 LieraOS Project
//
// Changing the power limit requires root. The write is executed via
// pkexec so the desktop shows a polkit authentication dialog instead of
// silently failing. NVIDIA uses nvidia-smi; AMD writes power1_cap.

use std::process::Command;

/// Set the GPU power limit in watts for the card at `pci_address`.
/// Returns an error string on failure so the GUI can show it.
pub fn set_power_limit(vendor_is_nvidia: bool, pci_address: &str, watts: u32) -> Result<(), String> {
    if vendor_is_nvidia {
        let bus_id = if pci_address.len() == 7 {
            format!("00000000:{}", pci_address)
        } else {
            pci_address.to_string()
        };
        let watts_str = watts.to_string();
        let out = Command::new("pkexec")
            .args(["nvidia-smi", "-i", &bus_id, "-pl", &watts_str])
            .output()
            .map_err(|e| format!("pkexec spawn failed: {}", e))?;

        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    } else {
        // AMD: power1_cap takes microwatts.
        let cap = crate::gpu_sysfs::hwmon_dir_for_pci(pci_address)
            .map(|h| h.join("power1_cap"))
            .filter(|p| p.exists())
            .ok_or_else(|| "power1_cap not found for this GPU".to_string())?;

        let microwatts = watts.saturating_mul(1_000_000).to_string();
        let out = Command::new("pkexec")
            .args([
                "sh",
                "-c",
                &format!("echo {} > {}", microwatts, cap.display()),
            ])
            .output()
            .map_err(|e| format!("pkexec spawn failed: {}", e))?;

        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}
