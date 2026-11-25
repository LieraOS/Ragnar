// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-shared — constants for D-Bus names, paths, config locations
// Copyright (C) 2026 LieraOS Project

/// D-Bus bus name for the display service daemon.
pub const DBUS_DISPLAY_SERVICE: &str = "org.lieraos.ragnar.DisplayServices";

/// D-Bus object path for the display service.
pub const DBUS_DISPLAY_PATH: &str = "/org/lieraos/ragnar/DisplayServices";

/// Directory for ragnar config files (XDG_CONFIG_HOME or ~/.config/ragnar).
pub fn config_dir() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home).join(".config")
        });
    base.join("ragnar")
}

/// Path to the display config TOML file.
pub fn display_config_path() -> std::path::PathBuf {
    config_dir().join("display-config.toml")
}
