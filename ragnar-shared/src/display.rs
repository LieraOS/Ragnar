// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-shared — display types shared between daemon and GUI
// Copyright (C) 2026 LieraOS Project

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// A single Wayland output head (physical monitor connection).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Head {
    /// Connector name from the compositor (e.g. "DP-3", "HDMI-A-1").
    pub name: String,
    /// Human-readable model name (e.g. "Philips 245P").
    pub model: String,
    /// Manufacturer (e.g. "Philips").
    pub manufacturer: String,
    /// Serial number for persistent identification across reboots.
    pub serial: String,
    /// Whether the output is currently enabled (has a mode set).
    pub enabled: bool,
    /// Currently active mode. If disabled, width/height are 0.
    pub current_mode: Mode,
    /// All modes the monitor advertises (for resolution/refresh pickers).
    pub modes: Vec<Mode>,
    /// Physical position in the global layout (x, y in pixels).
    pub position: (i32, i32),
    /// Whether this is the primary output (panel/taskbar follows).
    pub primary: bool,
    /// Scale factor (1.0 = native, 2.0 = hidpi).
    pub scale: f64,
    /// Rotation in degrees (0, 90, 180, 270).
    pub rotation: i32,
    /// Whether adaptive sync (VRR / FreeSync / G-Sync) is enabled.
    pub adaptive_sync: bool,
}

/// A display mode (resolution + refresh rate).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct Mode {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// Refresh rate in millihertz (e.g. 59950 = 59.95 Hz).
    pub refresh_rate: i32,
    /// Whether this is the preferred mode for the monitor.
    pub preferred: bool,
}

impl Mode {
    /// Format refresh rate as Hz string (e.g. "59.95 Hz").
    pub fn refresh_hz_string(&self) -> String {
        let hz = self.refresh_rate as f64 / 1000.0;
        format!("{:.2} Hz", hz)
    }

    /// Format resolution as "WxH" string.
    pub fn resolution_string(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

/// A stored display profile matching a specific set of connected monitors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Unique name for this profile.
    pub name: String,
    /// Whether this profile is preferred (applied automatically on match).
    pub preferred: bool,
    /// Monitor identifiers (serial numbers) this profile matches.
    pub identifiers: Vec<String>,
    /// Which output is the primary in this profile.
    pub primary_output: Option<String>,
    /// Per-output configuration in this profile.
    pub outputs: Vec<OutputConfig>,
}

/// Configuration for a single output within a profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Connector name (e.g. "DP-3").
    pub name: String,
    /// Whether this output is enabled in the profile.
    pub enabled: bool,
    /// Mode to use (width, height, refresh).
    pub mode: Option<Mode>,
    /// Position in the layout (x, y).
    pub position: (i32, i32),
    /// Scale factor.
    pub scale: f64,
    /// Rotation in degrees.
    pub rotation: i32,
    /// Adaptive sync on/off.
    pub adaptive_sync: bool,
}

/// The complete display configuration file (TOML).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisplayConfig {
    /// Global preferences.
    pub preferences: Preferences,
    /// Stored profiles.
    pub groups: Vec<Profile>,
}

/// Global display preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preferences {
    /// How to handle outputs not in any stored profile.
    /// "none" = no automatic arrangement,
    /// "side_by_side" = arrange left-to-right.
    pub automatic_attach_outputs_relative_position: String,
}
