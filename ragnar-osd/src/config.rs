// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-osd — user configuration
//
// Same decoupled file-based pattern as ragnar-raven: Ragnar
// Settings writes ~/.config/ragnar-osd/settings.ini, this service
// reads the [osd] section and watches the file for live changes —
// no D-Bus round-trip needed for toggles.
//
// Keys (all optional, defaults in parentheses):
//   enabled    = true|false   (true)  — master switch
//   duration_s = seconds      (2.5)   — how long the OSD stays up
//   scale      = percent      (100)   — OSD size, 50..200 %

use gio::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    /// Current configuration — updated live by the file monitor.
    static CFG: Cell<Config> = Cell::new(Config::default());
}

/// The three knobs the settings panel exposes.
#[derive(Clone, Copy)]
struct Config {
    enabled: bool,
    duration_ms: u32,
    scale: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: 2500,
            scale: 100,
        }
    }
}

/// The config file path.
fn ini_path() -> PathBuf {
    let mut p = glib::user_config_dir();
    p.push("ragnar-osd");
    p.push("settings.ini");
    p
}

/// Loads the [osd] section into the cached config.
pub fn load() {
    let mut cfg = Config::default();
    if let Ok(s) = std::fs::read_to_string(ini_path()) {
        let mut in_sec = false;
        for line in s.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                in_sec = t == "[osd]";
            } else if in_sec {
                if let Some((k, v)) = t.split_once('=') {
                    let v = v.trim();
                    match k.trim() {
                        "enabled" => cfg.enabled = v != "false",
                        "duration_s" => {
                            if let Ok(d) = v.parse::<f64>() {
                                cfg.duration_ms = (d.clamp(0.5, 30.0) * 1000.0) as u32;
                            }
                        }
                        "scale" => {
                            if let Ok(p) = v.parse::<u32>() {
                                cfg.scale = p.clamp(50, 200);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    CFG.with(|c| c.set(cfg));
}

/// Watches the config file and reloads on every change — the
/// settings panel writes, the next OSD uses the new values.
pub fn watch() {
    let file = gio::File::for_path(ini_path());
    let Ok(mon) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) else {
        return;
    };
    mon.connect_changed(|_, _, _, event| {
        if event == gio::FileMonitorEvent::ChangesDoneHint {
            load();
        }
    });
    std::mem::forget(mon);
}

/// Whether the OSD may show at all.
pub fn enabled() -> bool {
    CFG.with(|c| c.get().enabled)
}

/// Auto-hide time in milliseconds.
pub fn duration_ms() -> u32 {
    CFG.with(|c| c.get().duration_ms)
}

/// Size scale in percent (50..200).
pub fn scale() -> u32 {
    CFG.with(|c| c.get().scale)
}
