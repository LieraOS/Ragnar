// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — D-Bus service implementation
// Copyright (C) 2026 LieraOS Project
//
// Exposes the org.lieraos.ragnar.DisplayServices D-Bus interface.
// Methods apply changes via wlr-randr and persist to TOML config.

use std::collections::HashMap;
use std::process::Command;

use ragnar_shared::dbus_display::*;
use ragnar_shared::display::Head;
use ragnar_shared::gpu::GetGpusReply;

/// The D-Bus service object. Methods are called by the GUI.
pub struct DisplayService {
    /// Cached list of currently connected heads.
    heads: Vec<Head>,
    /// Pending position changes: connector name → (x, y) in real pixels.
    pending_positions: HashMap<String, (i32, i32)>,
    /// Pending mode changes: connector name → mode. Applied on apply_config().
    pending_modes: HashMap<String, ragnar_shared::display::Mode>,
    /// Pending scale changes: connector name → factor. Applied on apply_config().
    pending_scales: HashMap<String, f64>,
    /// Pending rotation changes: connector name → degrees. Applied on apply_config().
    pending_rotations: HashMap<String, i32>,
    /// Pending adaptive-sync changes: connector name → on/off. Applied on apply_config().
    pending_sync: HashMap<String, bool>,
}

impl DisplayService {
    /// Create a new service instance with the given heads.
    pub fn new(heads: Vec<Head>) -> Self {
        DisplayService {
            heads,
            pending_positions: HashMap::new(),
            pending_modes: HashMap::new(),
            pending_scales: HashMap::new(),
            pending_rotations: HashMap::new(),
            pending_sync: HashMap::new(),
        }
    }
}

#[zbus::interface(name = "org.lieraos.ragnar.DisplayServices")]
impl DisplayService {
    /// Return all currently connected outputs.
    fn get_outputs(&self) -> GetOutputsReply {
        GetOutputsReply {
            heads: self.heads.clone(),
        }
    }

    /// Store a new position for an output. Applied on apply_config().
    fn set_position(&mut self, output: String, x: i32, y: i32) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_position {} -> ({}, {})",
            output, x, y
        );
        self.pending_positions.insert(output, (x, y));
        ResultReply {
            success: true,
            message: "position queued".to_string(),
        }
    }

    /// Set which output is the primary.
    async fn set_primary(
        &mut self,
        output: String,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> ResultReply {
        eprintln!("ragnar-display-svc: set_primary {}", output);
        for head in &mut self.heads {
            head.primary = head.name == output;
        }
        // Notify listeners (panel etc.) so they can react live.
        let _ = emitter.primary_changed(&output).await;
        ResultReply {
            success: true,
            message: "primary set".to_string(),
        }
    }

    /// Signal emitted when the primary output changes.
    #[zbus(signal)]
    async fn primary_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        output: &str,
    ) -> zbus::Result<()>;

    /// Change the mode (resolution/refresh) of an output.
    fn set_mode(&mut self, output: String, width: i32, height: i32, refresh_rate: i32) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_mode {} {}x{} @ {}mHz",
            output, width, height, refresh_rate
        );
        // Queue the mode; it is applied to the compositor on apply_config.
        for head in &mut self.heads {
            if head.name == output {
                head.current_mode.width = width;
                head.current_mode.height = height;
                head.current_mode.refresh_rate = refresh_rate;
            }
        }
        self.pending_modes.insert(
            output,
            ragnar_shared::display::Mode {
                width,
                height,
                refresh_rate,
                preferred: false,
            },
        );
        ResultReply {
            success: true,
            message: "mode queued".to_string(),
        }
    }

    /// Queue a new scale factor for an output. Applied on apply_config().
    fn set_scale(&mut self, output: String, scale: f64) -> ResultReply {
        eprintln!("ragnar-display-svc: set_scale {} = {}", output, scale);
        for head in &mut self.heads {
            if head.name == output {
                head.scale = scale;
            }
        }
        self.pending_scales.insert(output, scale);
        ResultReply {
            success: true,
            message: "scale queued".to_string(),
        }
    }

    /// Queue a rotation (0/90/180/270 degrees). Applied on apply_config().
    fn set_rotation(&mut self, output: String, rotation: i32) -> ResultReply {
        eprintln!("ragnar-display-svc: set_rotation {} = {}°", output, rotation);
        let rotation = match rotation {
            90 | 180 | 270 => rotation,
            _ => 0,
        };
        for head in &mut self.heads {
            if head.name == output {
                head.rotation = rotation;
            }
        }
        self.pending_rotations.insert(output, rotation);
        ResultReply {
            success: true,
            message: "rotation queued".to_string(),
        }
    }

    /// Queue adaptive-sync on/off (VRR / FreeSync / G-Sync). Applied on apply_config().
    fn set_adaptive_sync(&mut self, output: String, enabled: bool) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_adaptive_sync {} = {}",
            output, enabled
        );
        for head in &mut self.heads {
            if head.name == output {
                head.adaptive_sync = enabled;
            }
        }
        self.pending_sync.insert(output, enabled);
        ResultReply {
            success: true,
            message: "adaptive sync queued".to_string(),
        }
    }

    /// Enable or disable an output.
    fn enable_output(&mut self, output: String, enabled: bool) -> ResultReply {
        eprintln!("ragnar-display-svc: enable_output {} = {}", output, enabled);
        for head in &mut self.heads {
            if head.name == output {
                head.enabled = enabled;
            }
        }
        ResultReply {
            success: true,
            message: "enabled set".to_string(),
        }
    }

    /// Apply all pending changes to the compositor via wlr-randr,
    /// then save the configuration to TOML.
    fn apply_config(&mut self) -> ResultReply {
        eprintln!("ragnar-display-svc: apply_config");

        // Apply pending positions via wlr-randr.
        for (output, (x, y)) in &self.pending_positions {
            let result = Command::new("wlr-randr")
                .args(["--output", output, "--pos", &format!("{},{}", x, y)])
                .output();

            match result {
                Ok(out) if out.status.success() => {
                    eprintln!("  wlr-randr: {} -> ({}, {}) OK", output, x, y);
                    // Update cached head position.
                    for head in &mut self.heads {
                        if head.name == *output {
                            head.position = (*x, *y);
                        }
                    }
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr failed for {}: {}", output, stderr.trim()),
                    };
                }
                Err(e) => {
                    return ResultReply {
                        success: false,
                        message: format!("failed to run wlr-randr: {}", e),
                    };
                }
            }
        }

        // Apply pending modes via wlr-randr (--mode WxH@Hz).
        for (output, mode) in &self.pending_modes {
            let mode_arg = format!(
                "{}x{}@{:.3}Hz",
                mode.width,
                mode.height,
                mode.refresh_rate as f64 / 1000.0
            );
            let result = Command::new("wlr-randr")
                .args(["--output", output, "--mode", &mode_arg])
                .output();
            match result {
                Ok(out) if out.status.success() => {
                    eprintln!("  wlr-randr: {} -> {} OK", output, mode_arg);
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr mode failed for {}: {}", output, stderr.trim()),
                    };
                }
                Err(e) => {
                    return ResultReply {
                        success: false,
                        message: format!("failed to run wlr-randr: {}", e),
                    };
                }
            }
        }

        // Apply pending scale factors via wlr-randr (--scale).
        for (output, scale) in &self.pending_scales {
            let result = Command::new("wlr-randr")
                .args(["--output", output, "--scale", &scale.to_string()])
                .output();
            match result {
                Ok(out) if out.status.success() => {
                    eprintln!("  wlr-randr: {} -> scale {} OK", output, scale);
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr scale failed for {}: {}", output, stderr.trim()),
                    };
                }
                Err(e) => {
                    return ResultReply {
                        success: false,
                        message: format!("failed to run wlr-randr: {}", e),
                    };
                }
            }
        }

        // Apply pending rotations via wlr-randr (--transform).
        for (output, rotation) in &self.pending_rotations {
            let transform = match rotation {
                90 => "90",
                180 => "180",
                270 => "270",
                _ => "normal",
            };
            let result = Command::new("wlr-randr")
                .args(["--output", output, "--transform", transform])
                .output();
            match result {
                Ok(out) if out.status.success() => {
                    eprintln!("  wlr-randr: {} -> transform {} OK", output, transform);
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr transform failed for {}: {}", output, stderr.trim()),
                    };
                }
                Err(e) => {
                    return ResultReply {
                        success: false,
                        message: format!("failed to run wlr-randr: {}", e),
                    };
                }
            }
        }

        // Apply pending adaptive-sync flags via wlr-randr.
        for (output, enabled) in &self.pending_sync {
            let flag = if *enabled { "enabled" } else { "disabled" };
            let result = Command::new("wlr-randr")
                .args(["--output", output, "--adaptive-sync", flag])
                .output();
            match result {
                Ok(out) if out.status.success() => {
                    eprintln!("  wlr-randr: {} -> adaptive-sync {} OK", output, flag);
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr adaptive-sync failed for {}: {}", output, stderr.trim()),
                    };
                }
                Err(e) => {
                    return ResultReply {
                        success: false,
                        message: format!("failed to run wlr-randr: {}", e),
                    };
                }
            }
        }

        // Clear all pending changes after successful apply.
        self.pending_positions.clear();
        self.pending_modes.clear();
        self.pending_scales.clear();
        self.pending_rotations.clear();
        self.pending_sync.clear();

        // Notify the Ragnar panel which output is primary.
        // The panel watches this GSettings key and moves the taskbar live.
        // X11 clients (Steam, Proton) get the same answer via RandR primary.
        if let Some(head) = self.heads.iter().find(|h| h.primary) {
            crate::gsettings::write_primary_monitor(&head.name);
            crate::xrandr::set_primary(&head.name);
        }

        // Save configuration to TOML for persistence across reboots.
        match save_config(&self.heads) {
            Ok(()) => {
                eprintln!("ragnar-display-svc: config saved");
                ResultReply {
                    success: true,
                    message: "applied and saved".to_string(),
                }
            }
            Err(e) => {
                eprintln!("ragnar-display-svc: failed to save config: {}", e);
                ResultReply {
                    success: true,
                    message: format!("applied but save failed: {}", e),
                }
            }
        }
    }

    /// Return detected GPUs with live sensor data.
    fn get_gpus(&self) -> GetGpusReply {
        GetGpusReply {
            gpus: crate::gpu::detect_gpus(),
        }
    }

    /// Set the power limit of a GPU (requires polkit auth via pkexec).
    fn set_gpu_power_limit(&self, pci_address: String, watts: f64) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_gpu_power_limit {} -> {}W",
            pci_address, watts
        );
        // Detect vendor from sysfs so the right write path is used.
        let is_nvidia = std::fs::read_to_string(format!(
            "/sys/bus/pci/devices/{}/vendor",
            pci_address
        ))
        .map(|v| v.trim() == "0x10de")
        .unwrap_or(false);

        match crate::gpu_power::set_power_limit(is_nvidia, &pci_address, watts as u32) {
            Ok(()) => ResultReply {
                success: true,
                message: "power limit set".to_string(),
            },
            Err(e) => ResultReply {
                success: false,
                message: e,
            },
        }
    }

    /// Set the fan speed in percent for a GPU (AMD via pwm1; polkit).
    fn set_gpu_fan(&self, pci_address: String, percent: i32) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_gpu_fan {} -> {}%",
            pci_address, percent
        );
        match crate::gpu_fan::set_fan_percent(&pci_address, percent.clamp(0, 100) as u8) {
            Ok(()) => ResultReply {
                success: true,
                message: "fan speed set".to_string(),
            },
            Err(e) => ResultReply {
                success: false,
                message: e,
            },
        }
    }

    /// Lock the NVIDIA graphics clock to a fixed MHz (0 = unlock).
    fn set_gpu_locked_clock(&self, pci_address: String, mhz: i32) -> ResultReply {
        eprintln!(
            "ragnar-display-svc: set_gpu_locked_clock {} -> {}MHz",
            pci_address, mhz
        );
        match crate::gpu_clock::set_locked_clock_mhz(&pci_address, mhz.max(0) as u32) {
            Ok(()) => ResultReply {
                success: true,
                message: if mhz == 0 {
                    "clock lock cleared".to_string()
                } else {
                    "clock locked".to_string()
                },
            },
            Err(e) => ResultReply {
                success: false,
                message: e,
            },
        }
    }

    /// Reset to automatic layout (side-by-side).
    fn reset_config(&self) -> ResultReply {
        eprintln!("ragnar-display-svc: reset_config");
        let mut x = 0;
        for head in &self.heads {
            if !head.enabled {
                continue;
            }
            let result = Command::new("wlr-randr")
                .args(["--output", &head.name, "--pos", &format!("{},0", x)])
                .output();
            if let Ok(out) = result {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return ResultReply {
                        success: false,
                        message: format!("wlr-randr failed for {}: {}", head.name, stderr.trim()),
                    };
                }
            }
            x += head.current_mode.width;
        }
        ResultReply {
            success: true,
            message: "reset to side-by-side".to_string(),
        }
    }
}

/// Save the current head layout to the TOML config file.
fn save_config(heads: &[Head]) -> std::io::Result<()> {
    use ragnar_shared::config;
    use ragnar_shared::display::*;

    let mut outputs = Vec::new();
    for head in heads {
        if !head.enabled {
            continue;
        }
        outputs.push(OutputConfig {
            name: head.name.clone(),
            enabled: head.enabled,
            mode: Some(Mode {
                width: head.current_mode.width,
                height: head.current_mode.height,
                refresh_rate: head.current_mode.refresh_rate,
                preferred: head.current_mode.preferred,
            }),
            position: head.position,
            scale: head.scale,
            rotation: head.rotation,
            adaptive_sync: head.adaptive_sync,
        });
    }

    let identifiers: Vec<String> = heads
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.serial.clone())
        .collect();

    let primary = heads.iter().find(|h| h.primary).map(|h| h.serial.clone());

    let profile = Profile {
        name: format!("{} (Auto Generated)", identifiers.join(", ")),
        preferred: true,
        identifiers,
        primary_output: primary,
        outputs,
    };

    let config = DisplayConfig {
        preferences: Preferences {
            automatic_attach_outputs_relative_position: "side_by_side".to_string(),
        },
        groups: vec![profile],
    };

    config::save(&config)
}
