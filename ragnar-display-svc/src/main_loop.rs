// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — main event loop: D-Bus service
// Copyright (C) 2026 LieraOS Project
//
// Detects outputs via wlr-randr, loads saved config, applies the
// matching profile, registers the D-Bus service, and keeps running.

use std::process::Command;

use crate::config;
use crate::dbus_service::DisplayService;
use crate::wayland;
use ragnar_shared::constants;
use ragnar_shared::display::DisplayConfig;
use zbus::Connection as ZbusConnection;

/// Entry point: detect outputs, load config, apply profile,
/// register D-Bus service.
pub fn run() {
    eprintln!("ragnar-display-svc: starting");

    let display_config = config::load();
    eprintln!(
        "ragnar-display-svc: loaded config with {} profiles",
        display_config.groups.len()
    );

    let mut heads = wayland::detect_outputs();
    eprintln!(
        "ragnar-display-svc: detected {} outputs",
        heads.len()
    );

    for head in &heads {
        eprintln!(
            "  {} {} {}x{} @ {} pos ({},{}) serial={}",
            head.name,
            head.model,
            head.current_mode.width,
            head.current_mode.height,
            head.current_mode.refresh_hz_string(),
            head.position.0,
            head.position.1,
            head.serial,
        );
    }

    // Apply the matching saved profile, if any.
    apply_saved_profile(&display_config, &mut heads);

    // Start D-Bus service (blocking — runs until process is killed).
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    rt.block_on(async {
        let dbus_conn = ZbusConnection::session()
            .await
            .expect("failed to connect to D-Bus session bus");

        let service = DisplayService::new(heads);
        dbus_conn
            .object_server()
            .at(constants::DBUS_DISPLAY_PATH, service)
            .await
            .expect("failed to register D-Bus object");

        dbus_conn
            .request_name(constants::DBUS_DISPLAY_SERVICE)
            .await
            .expect("failed to acquire D-Bus name");

        eprintln!(
            "ragnar-display-svc: D-Bus service registered as {}",
            constants::DBUS_DISPLAY_SERVICE
        );

        std::future::pending::<()>().await;
    });
}

/// Find the saved profile matching the currently connected monitors
/// and apply its positions via wlr-randr.
fn apply_saved_profile(config: &DisplayConfig, heads: &mut Vec<ragnar_shared::display::Head>) {
    // Collect serials of currently connected monitors.
    let connected_serials: Vec<&str> = heads
        .iter()
        .filter(|h| h.enabled)
        .map(|h| h.serial.as_str())
        .collect();

    // Find a profile whose identifier list matches the connected serials.
    let profile = config.groups.iter().find(|p| {
        p.identifiers.len() == connected_serials.len()
            && p.identifiers
                .iter()
                .all(|id| connected_serials.contains(&id.as_str()))
    });

    let profile = match profile {
        Some(p) => p,
        None => {
            eprintln!("ragnar-display-svc: no matching profile — keeping current layout");
            return;
        }
    };

    eprintln!(
        "ragnar-display-svc: applying profile '{}' (preferred={})",
        profile.name, profile.preferred
    );

    // Apply each output config via wlr-randr.
    for output_cfg in &profile.outputs {
        let head = heads.iter_mut().find(|h| h.serial == output_cfg.name
            || h.name == output_cfg.name);
        let head = match head {
            Some(h) => h,
            None => continue,
        };

        // Skip disabled outputs.
        if !output_cfg.enabled {
            let _ = Command::new("wlr-randr")
                .args(["--output", &head.name, "--off"])
                .output();
            continue;
        }

        // Set position.
        let pos = format!("{},{}", output_cfg.position.0, output_cfg.position.1);
        let result = Command::new("wlr-randr")
            .args(["--output", &head.name, "--pos", &pos])
            .output();

        match result {
            Ok(out) if out.status.success() => {
                eprintln!(
                    "  {} -> ({},{}) applied",
                    head.name, output_cfg.position.0, output_cfg.position.1
                );
                head.position = output_cfg.position;
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!("  {} -> wlr-randr failed: {}", head.name, stderr.trim());
            }
            Err(e) => {
                eprintln!("  {} -> failed to run wlr-randr: {}", head.name, e);
            }
        }

        // Set mode (resolution + refresh) when the profile stores one.
        if let Some(mode) = &output_cfg.mode {
            let mode_arg = format!(
                "{}x{}@{:.3}Hz",
                mode.width,
                mode.height,
                mode.refresh_rate as f64 / 1000.0
            );
            let _ = Command::new("wlr-randr")
                .args(["--output", &head.name, "--mode", &mode_arg])
                .output();
            head.current_mode = mode.clone();
        }

        // Set scale factor.
        if output_cfg.scale != 1.0 {
            let _ = Command::new("wlr-randr")
                .args(["--output", &head.name, "--scale", &output_cfg.scale.to_string()])
                .output();
        }
        head.scale = output_cfg.scale;

        // Set rotation.
        let transform = match output_cfg.rotation {
            90 => "90",
            180 => "180",
            270 => "270",
            _ => "normal",
        };
        if output_cfg.rotation != 0 {
            let _ = Command::new("wlr-randr")
                .args(["--output", &head.name, "--transform", transform])
                .output();
        }
        head.rotation = output_cfg.rotation;

        // Set adaptive sync.
        let flag = if output_cfg.adaptive_sync { "enabled" } else { "disabled" };
        let _ = Command::new("wlr-randr")
            .args(["--output", &head.name, "--adaptive-sync", flag])
            .output();
        head.adaptive_sync = output_cfg.adaptive_sync;

        // Set primary — profile stores the serial number.
        head.primary = profile
            .primary_output
            .as_deref()
            .map(|p| p == head.serial)
            .unwrap_or(false);
    }

    // Propagate the restored primary to the panel (GSettings) and to
    // XWayland (RandR primary) so X11 clients see the same primary output.
    if let Some(head) = heads.iter().find(|h| h.primary) {
        crate::gsettings::write_primary_monitor(&head.name);
        crate::xrandr::set_primary(&head.name);
    }
}
