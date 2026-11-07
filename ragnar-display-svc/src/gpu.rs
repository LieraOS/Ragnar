// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — GPU detection dispatch
// Copyright (C) 2026 LieraOS Project
//
// Enumerates DRM card devices and dispatches to the vendor-specific
// query implementation. NVIDIA is queried via nvidia-smi (works as
// unprivileged user for reads); AMD/Intel are read from sysfs/hwmon.

use std::fs;
use std::path::Path;

use ragnar_shared::gpu::{GpuInfo, GpuVendor};

use crate::gpu_nvidia;
use crate::gpu_sysfs;

/// Detect all GPUs present under /sys/class/drm/cardN.
pub fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let drm = match fs::read_dir("/sys/class/drm") {
        Ok(d) => d,
        Err(_) => return gpus,
    };

    for entry in drm.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Only real cards, not connector sub-entries like card1-DP-3.
        let index = match name
            .strip_prefix("card")
            .and_then(|rest| rest.parse::<i32>().ok())
        {
            Some(i) => i,
            None => continue,
        };

        let device = entry.path().join("device");
        let vendor_raw = match fs::read_to_string(device.join("vendor")) {
            Ok(v) => v.trim().to_string(),
            Err(_) => continue,
        };

        let vendor = match vendor_raw.as_str() {
            "0x10de" => GpuVendor::Nvidia,
            "0x1002" | "0x1022" => GpuVendor::Amd,
            "0x8086" => GpuVendor::Intel,
            _ => GpuVendor::Unknown,
        };

        let pci_address = pci_address_of(&device);
        let driver = driver_module_of(&device);

        let info = match vendor {
            GpuVendor::Nvidia => gpu_nvidia::query(index, &pci_address, &driver),
            _ => gpu_sysfs::query(index, vendor, &pci_address, &driver, &device),
        };

        if let Some(i) = info {
            gpus.push(i);
        }
    }

    gpus
}

/// Resolve the PCI bus address from the card device symlink target.
fn pci_address_of(device: &Path) -> String {
    device
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default()
}

/// Resolve the bound kernel driver module name (e.g. "nvidia", "amdgpu").
fn driver_module_of(device: &Path) -> String {
    device
        .join("driver")
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default()
}
