// Copyright (C) Jörg Schröder
//! Balance Control
//!
//! Channel balance control for surround sound (5.1: front, rear, subwoofer).

use serde::{Deserialize, Serialize};
use tracing::debug;

/// Balance settings for surround sound
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceControl {
    /// Front left volume (0.0 to 1.0)
    pub front_left: f32,
    /// Front right volume (0.0 to 1.0)
    pub front_right: f32,
    /// Rear left volume (0.0 to 1.0)
    pub rear_left: f32,
    /// Rear right volume (0.0 to 1.0)
    pub rear_right: f32,
    /// Center volume (0.0 to 1.0)
    pub center: f32,
    /// Subwoofer/LFE volume (0.0 to 1.0)
    pub subwoofer: f32,
}

impl Default for BalanceControl {
    fn default() -> Self {
        Self {
            front_left: 1.0,
            front_right: 1.0,
            rear_left: 1.0,
            rear_right: 1.0,
            center: 1.0,
            subwoofer: 1.0,
        }
    }
}

impl BalanceControl {
    /// Create a new balance control with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all channels to default (1.0)
    pub fn reset(&mut self) {
        *self = Self::default();
        debug!("Balance control reset to defaults");
    }

    /// Set front balance (left/right)
    ///
    /// # Arguments
    ///
    /// * `balance` - Balance value (-1.0 to 1.0, where 0.0 is centered)
    pub fn set_front_balance(&mut self, balance: f32) {
        let balance = balance.clamp(-1.0, 1.0);
        if balance >= 0.0 {
            self.front_left = 1.0 - balance;
            self.front_right = 1.0;
        } else {
            self.front_left = 1.0;
            self.front_right = 1.0 + balance;
        }
        debug!("Front balance set to {}", balance);
    }

    /// Set rear balance (left/right)
    ///
    /// # Arguments
    ///
    /// * `balance` - Balance value (-1.0 to 1.0, where 0.0 is centered)
    pub fn set_rear_balance(&mut self, balance: f32) {
        let balance = balance.clamp(-1.0, 1.0);
        if balance >= 0.0 {
            self.rear_left = 1.0 - balance;
            self.rear_right = 1.0;
        } else {
            self.rear_left = 1.0;
            self.rear_right = 1.0 + balance;
        }
        debug!("Rear balance set to {}", balance);
    }

    /// Set center volume
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume (0.0 to 1.0)
    pub fn set_center_volume(&mut self, volume: f32) {
        self.center = volume.clamp(0.0, 1.0);
        debug!("Center volume set to {}", self.center);
    }

    /// Set subwoofer volume
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume (0.0 to 1.0)
    pub fn set_subwoofer_volume(&mut self, volume: f32) {
        self.subwoofer = volume.clamp(0.0, 1.0);
        debug!("Subwoofer volume set to {}", self.subwoofer);
    }

    /// Get front balance as a single value
    ///
    /// Returns -1.0 (full left) to 1.0 (full right), 0.0 is centered
    pub fn front_balance(&self) -> f32 {
        if self.front_left > self.front_right {
            -(1.0 - self.front_right)
        } else {
            self.front_left - 1.0
        }
    }

    /// Get rear balance as a single value
    ///
    /// Returns -1.0 (full left) to 1.0 (full right), 0.0 is centered
    pub fn rear_balance(&self) -> f32 {
        if self.rear_left > self.rear_right {
            -(1.0 - self.rear_right)
        } else {
            self.rear_left - 1.0
        }
    }
}
