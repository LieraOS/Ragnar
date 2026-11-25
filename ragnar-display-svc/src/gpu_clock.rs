// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — NVIDIA locked GPU clocks
// Copyright (C) 2026 LieraOS Project
//
// nvidia-smi -lgc locks the graphics clock to a fixed frequency. This is
// not overclocking (offsets need X11 NV-CONTROL, unavailable on Wayland)
// but it is the supported way to pin clocks — useful to keep boost clocks
// stable or limit them for thermals/noise. Pass 0 to unlock (-rgc).

use std::process::Command;

/// Lock the graphics clock of the NVIDIA GPU at `pci_address` to `mhz`.
/// `mhz == 0` resets the lock so the GPU boosts freely again.
pub fn set_locked_clock_mhz(pci_address: &str, mhz: u32) -> Result<(), String> {
    let bus_id = if pci_address.len() == 7 {
        format!("00000000:{}", pci_address)
    } else {
        pci_address.to_string()
    };

    // -rgc resets the lock, -lgc <min>-<max> pins a fixed frequency.
    let args: Vec<String> = if mhz == 0 {
        vec!["-i".into(), bus_id, "-rgc".into()]
    } else {
        vec!["-i".into(), bus_id, "-lgc".into(), format!("{0}-{0}", mhz)]
    };

    let out = Command::new("pkexec")
        .args(["nvidia-smi"])
        .args(&args)
        .output()
        .map_err(|e| format!("pkexec spawn failed: {}", e))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
