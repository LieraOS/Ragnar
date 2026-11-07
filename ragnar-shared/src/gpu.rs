// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-shared — GPU information types shared between daemon and GUI
// Copyright (C) 2026 LieraOS Project

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Vendor of a detected GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

/// Runtime information and capabilities of one GPU.
/// Fields that a driver does not expose are reported as 0/empty.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GpuInfo {
    /// Kernel card index (e.g. 1 for /sys/class/drm/card1).
    pub card_index: i32,
    /// Detected vendor.
    pub vendor: GpuVendor,
    /// Marketing name (e.g. "NVIDIA GeForce GTX 1060 6GB").
    pub name: String,
    /// Driver version string (e.g. "580.178.04") or module name (e.g. "amdgpu").
    pub driver: String,
    /// PCI bus address (e.g. "0000:01:00.0"), used to target nvidia-smi -i.
    pub pci_address: String,
    /// Core temperature in degrees Celsius (-1 when unknown).
    pub temperature_c: i32,
    /// Current power draw in watts.
    pub power_draw_w: f64,
    /// Current power limit in watts.
    pub power_limit_w: f64,
    /// Minimum allowed power limit in watts.
    pub power_limit_min_w: f64,
    /// Maximum allowed power limit in watts.
    pub power_limit_max_w: f64,
    /// Fan speed in percent (-1 when unknown or fanless).
    pub fan_percent: i32,
    /// Current graphics clock in MHz.
    pub core_clock_mhz: i32,
    /// Current memory clock in MHz.
    pub mem_clock_mhz: i32,
    /// Maximum supported graphics clock in MHz (0 when unknown).
    pub max_graphics_clock_mhz: i32,
    /// Whether the power limit can be changed by the user.
    pub supports_power_limit: bool,
    /// Whether a fan curve / fan speed can be set.
    pub supports_fan_control: bool,
    /// Whether clock offsets (overclocking) can be applied.
    pub supports_overclock: bool,
}

/// Return value for GetGpus: list of detected GPUs.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetGpusReply {
    pub gpus: Vec<GpuInfo>,
}

/// Argument for SetGpuPowerLimit.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SetGpuPowerLimitArgs {
    /// PCI bus address identifying the GPU (e.g. "0000:01:00.0").
    pub pci_address: String,
    /// New power limit in watts.
    pub watts: f64,
}
