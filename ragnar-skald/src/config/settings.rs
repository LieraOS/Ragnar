// Copyright (C) Jörg Schröder
//! Application Settings
//!
//! Application settings structure.

use crate::audio::config::AudioConfig;
use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Selected output device
    pub output_device: Option<String>,
    /// Audio configuration
    pub audio_config: AudioConfig,
    /// Window width
    pub window_width: i32,
    /// Window height
    pub window_height: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_device: None,
            audio_config: AudioConfig::new(),
            window_width: 800,
            window_height: 600,
        }
    }
}

impl AppSettings {
    /// Create new application settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the output device
    pub fn set_output_device(&mut self, device_name: String) {
        self.output_device = Some(device_name);
    }

    /// Get the output device name
    pub fn output_device_name(&self) -> Option<&String> {
        self.output_device.as_ref()
    }

    /// Get mutable audio configuration
    pub fn audio_config_mut(&mut self) -> &mut AudioConfig {
        &mut self.audio_config
    }

    /// Get audio configuration
    pub fn audio_config(&self) -> &AudioConfig {
        &self.audio_config
    }
}
