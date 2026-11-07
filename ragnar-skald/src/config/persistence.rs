//! Configuration Persistence
//!
//! Save and load application settings to/from disk.

use crate::config::settings::AppSettings;
use anyhow::{Context, Result};
use dirs::config_dir;
use serde_json;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Configuration persistence manager
pub struct ConfigPersistence {
    config_path: PathBuf,
}

impl ConfigPersistence {
    /// Create a new config persistence manager
    pub fn new() -> Result<Self> {
        let config_dir = config_dir().context("Failed to get config directory")?;

        let app_config_dir = config_dir.join("ragnar-skald");
        // Migration from the pre-rename "lieraaudio" config dir — the
        // user's settings must survive the product rename untouched.
        let legacy = config_dir.join("lieraaudio");
        if legacy.is_dir() && !app_config_dir.exists() {
            let _ = fs::rename(&legacy, &app_config_dir);
        }

        // Create config directory if it doesn't exist
        if !app_config_dir.exists() {
            fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
            info!("Created config directory: {:?}", app_config_dir);
        }

        let config_path = app_config_dir.join("settings.json");

        debug!("Config path: {:?}", config_path);

        Ok(Self { config_path })
    }

    /// Save settings to disk
    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let json =
            serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

        fs::write(&self.config_path, json).context("Failed to write config file")?;

        info!("Settings saved to {:?}", self.config_path);
        Ok(())
    }

    /// Load settings from disk
    pub fn load(&self) -> Result<AppSettings> {
        if !self.config_path.exists() {
            warn!("Config file not found, using defaults");
            return Ok(AppSettings::new());
        }

        let json = fs::read_to_string(&self.config_path).context("Failed to read config file")?;

        let settings: AppSettings =
            serde_json::from_str(&json).context("Failed to deserialize settings")?;

        info!("Settings loaded from {:?}", self.config_path);
        Ok(settings)
    }

    /// Get the config file path
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

/// Convenience for callers that can propagate nothing: logs the error
/// and falls back to an in-memory config path instead of panicking.
impl Default for ConfigPersistence {
    fn default() -> Self {
        match Self::new() {
            Ok(p) => p,
            Err(e) => {
                warn!("Config persistence unavailable: {e:#}");
                Self {
                    config_path: PathBuf::from("/tmp/ragnar-skald-settings.json"),
                }
            }
        }
    }
}
