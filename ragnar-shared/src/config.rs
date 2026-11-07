// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-shared — config save/load helpers
// Copyright (C) 2026 LieraOS Project

use std::fs;
use std::io;

use crate::constants;
use crate::display::DisplayConfig;

/// Load the display config from the default path.
/// Returns Default if the file does not exist yet.
pub fn load() -> DisplayConfig {
    let path = constants::display_config_path();
    match fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => DisplayConfig::default(),
    }
}

/// Save the display config to the default path.
/// Creates the config directory if it does not exist.
pub fn save(config: &DisplayConfig) -> io::Result<()> {
    let path = constants::display_config_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    fs::write(&path, content)
}
