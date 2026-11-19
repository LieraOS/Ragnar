// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — D-Bus client for ragnar-display-svc
// Copyright (C) 2026 LieraOS Project
//
// Calls the org.lieraos.ragnar.DisplayServices D-Bus interface
// to query and update display configuration.

use ragnar_shared::dbus_display::{GetOutputsReply, ResultReply};
use ragnar_shared::display::Head;
use ragnar_shared::gpu::{GetGpusReply, GpuInfo};
use zbus::proxy;

/// D-Bus proxy for the display service.
#[proxy(
    interface = "org.lieraos.ragnar.DisplayServices",
    default_service = "org.lieraos.ragnar.DisplayServices",
    default_path = "/org/lieraos/ragnar/DisplayServices"
)]
trait DisplayServices {
    fn get_outputs(&self) -> zbus::Result<GetOutputsReply>;
    fn set_position(&self, output: String, x: i32, y: i32) -> zbus::Result<ResultReply>;
    fn set_primary(&self, output: String) -> zbus::Result<ResultReply>;
    fn set_mode(
        &self,
        output: String,
        width: i32,
        height: i32,
        refresh_rate: i32,
    ) -> zbus::Result<ResultReply>;
    fn set_scale(&self, output: String, scale: f64) -> zbus::Result<ResultReply>;
    fn set_rotation(&self, output: String, rotation: i32) -> zbus::Result<ResultReply>;
    fn set_adaptive_sync(&self, output: String, enabled: bool) -> zbus::Result<ResultReply>;
    fn enable_output(&self, output: String, enabled: bool) -> zbus::Result<ResultReply>;
    fn apply_config(&self) -> zbus::Result<ResultReply>;
    fn reset_config(&self) -> zbus::Result<ResultReply>;
    fn get_gpus(&self) -> zbus::Result<GetGpusReply>;
    fn set_gpu_power_limit(&self, pci_address: String, watts: f64) -> zbus::Result<ResultReply>;
    fn set_gpu_fan(&self, pci_address: String, percent: i32) -> zbus::Result<ResultReply>;
    fn set_gpu_locked_clock(&self, pci_address: String, mhz: i32) -> zbus::Result<ResultReply>;
}

/// Connect to the display service via D-Bus (blocking).
fn connect() -> Result<DisplayServicesProxyBlocking<'static>, zbus::Error> {
    let connection = zbus::blocking::Connection::session()?;
    DisplayServicesProxyBlocking::new(&connection)
}

/// Fetch all connected outputs from the daemon.
/// Returns an empty list if the daemon is not running.
pub fn fetch_outputs() -> Vec<Head> {
    match connect() {
        Ok(proxy) => match proxy.get_outputs() {
            Ok(reply) => reply.heads,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: GetOutputs failed: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            tracing::warn!(
                "ragnar-settings-gui: failed to connect to display service: {}",
                e
            );
            Vec::new()
        }
    }
}

/// Send a monitor's new position to the daemon.
pub fn send_position(name: &str, x: i32, y: i32) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_position(name.to_string(), x, y) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_position failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Tell the daemon which output is the primary.
pub fn send_primary(name: &str) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_primary(name.to_string()) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_primary failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Tell the daemon to enable or disable an output.
pub fn send_enable(name: &str, enabled: bool) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.enable_output(name.to_string(), enabled) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: enable_output failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Tell the daemon to apply the configuration to the compositor.
pub fn send_apply() -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.apply_config() {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: apply_config failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Fetch detected GPUs with live sensor data from the daemon.
/// Returns an empty list if the daemon is not running.
pub fn fetch_gpus() -> Vec<GpuInfo> {
    match connect() {
        Ok(proxy) => match proxy.get_gpus() {
            Ok(reply) => reply.gpus,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: GetGpus failed: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            tracing::warn!(
                "ragnar-settings-gui: failed to connect to display service: {}",
                e
            );
            Vec::new()
        }
    }
}

/// Ask the daemon to set a GPU power limit (polkit prompt via pkexec).
pub fn send_gpu_power_limit(pci_address: &str, watts: f64) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_gpu_power_limit(pci_address.to_string(), watts) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_gpu_power_limit failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Ask the daemon to set a GPU fan speed in percent (pkexec/polkit).
pub fn send_gpu_fan(pci_address: &str, percent: i32) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_gpu_fan(pci_address.to_string(), percent) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_gpu_fan failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Ask the daemon to lock/unlock the NVIDIA graphics clock (pkexec).
pub fn send_gpu_locked_clock(pci_address: &str, mhz: i32) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_gpu_locked_clock(pci_address.to_string(), mhz) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_gpu_locked_clock failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Queue a mode change (resolution + refresh) on the daemon.
pub fn send_mode(name: &str, w: i32, h: i32, rr: i32) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_mode(name.to_string(), w, h, rr) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_mode failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Queue a scale factor change on the daemon.
pub fn send_scale(name: &str, scale: f64) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_scale(name.to_string(), scale) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_scale failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Queue a rotation (0/90/180/270) on the daemon.
pub fn send_rotation(name: &str, rotation: i32) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_rotation(name.to_string(), rotation) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_rotation failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}

/// Queue an adaptive-sync change on the daemon.
pub fn send_adaptive_sync(name: &str, enabled: bool) -> ResultReply {
    match connect() {
        Ok(proxy) => match proxy.set_adaptive_sync(name.to_string(), enabled) {
            Ok(reply) => reply,
            Err(e) => {
                tracing::warn!("ragnar-settings-gui: set_adaptive_sync failed: {}", e);
                ResultReply {
                    success: false,
                    message: e.to_string(),
                }
            }
        },
        Err(e) => {
            tracing::warn!("ragnar-settings-gui: D-Bus connection failed: {}", e);
            ResultReply {
                success: false,
                message: e.to_string(),
            }
        }
    }
}
