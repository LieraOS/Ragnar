// Copyright (C) Jörg Schröder
//! Audio Effects
//!
//! Implementation of audio effects: Reverb, Echo, and Equalizer.

use serde::{Deserialize, Serialize};
use tracing::debug;

/// Audio effect types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectType {
    /// Reverb effect
    Reverb,
    /// Echo effect
    Echo,
    /// Equalizer effect
    Equalizer,
}

/// Trait for audio effects
pub trait AudioEffect {
    /// Get the effect type
    fn effect_type(&self) -> EffectType;

    /// Set the effect intensity (0-10)
    fn set_intensity(&mut self, intensity: u8) -> anyhow::Result<()>;

    /// Get the effect intensity
    fn intensity(&self) -> u8;

    /// Enable or disable the effect
    fn set_enabled(&mut self, enabled: bool);

    /// Check if the effect is enabled
    fn is_enabled(&self) -> bool;
}

/// Reverb effect settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverbEffect {
    /// Effect intensity (0-10)
    intensity: u8,
    /// Whether the effect is enabled
    enabled: bool,
}

impl ReverbEffect {
    /// Create a new reverb effect
    pub fn new() -> Self {
        Self {
            intensity: 0,
            enabled: false,
        }
    }
}

impl AudioEffect for ReverbEffect {
    fn effect_type(&self) -> EffectType {
        EffectType::Reverb
    }

    fn set_intensity(&mut self, intensity: u8) -> anyhow::Result<()> {
        if intensity > 10 {
            return Err(anyhow::anyhow!(
                "Reverb intensity must be between 0 and 10, got {}",
                intensity
            ));
        }
        self.intensity = intensity;
        debug!("Reverb intensity set to {}", intensity);
        Ok(())
    }

    fn intensity(&self) -> u8 {
        self.intensity
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("Reverb enabled: {}", enabled);
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for ReverbEffect {
    fn default() -> Self {
        Self::new()
    }
}

/// Echo effect settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoEffect {
    /// Effect intensity (0-10)
    intensity: u8,
    /// Whether the effect is enabled
    enabled: bool,
}

impl EchoEffect {
    /// Create a new echo effect
    pub fn new() -> Self {
        Self {
            intensity: 0,
            enabled: false,
        }
    }
}

impl AudioEffect for EchoEffect {
    fn effect_type(&self) -> EffectType {
        EffectType::Echo
    }

    fn set_intensity(&mut self, intensity: u8) -> anyhow::Result<()> {
        if intensity > 10 {
            return Err(anyhow::anyhow!(
                "Echo intensity must be between 0 and 10, got {}",
                intensity
            ));
        }
        self.intensity = intensity;
        debug!("Echo intensity set to {}", intensity);
        Ok(())
    }

    fn intensity(&self) -> u8 {
        self.intensity
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("Echo enabled: {}", enabled);
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for EchoEffect {
    fn default() -> Self {
        Self::new()
    }
}

/// Equalizer effect settings (Bass/Treble)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerEffect {
    /// Bass level (-10 to +10)
    bass: i8,
    /// Treble level (-10 to +10)
    treble: i8,
    /// Whether the effect is enabled
    enabled: bool,
}

impl EqualizerEffect {
    /// Create a new equalizer effect
    pub fn new() -> Self {
        Self {
            bass: 0,
            treble: 0,
            enabled: false,
        }
    }

    /// Set bass level
    pub fn set_bass(&mut self, bass: i8) -> anyhow::Result<()> {
        if !(-15..=15).contains(&bass) {
            return Err(anyhow::anyhow!(
                "Bass level must be between -15 and +15, got {}",
                bass
            ));
        }
        self.bass = bass;
        debug!("Bass level set to {}", bass);
        Ok(())
    }

    /// Get bass level
    pub fn bass(&self) -> i8 {
        self.bass
    }

    /// Set treble level
    pub fn set_treble(&mut self, treble: i8) -> anyhow::Result<()> {
        if !(-15..=15).contains(&treble) {
            return Err(anyhow::anyhow!(
                "Treble level must be between -15 and +15, got {}",
                treble
            ));
        }
        self.treble = treble;
        debug!("Treble level set to {}", treble);
        Ok(())
    }

    /// Get treble level
    pub fn treble(&self) -> i8 {
        self.treble
    }
}

impl AudioEffect for EqualizerEffect {
    fn effect_type(&self) -> EffectType {
        EffectType::Equalizer
    }

    fn set_intensity(&mut self, _intensity: u8) -> anyhow::Result<()> {
        // Equalizer uses bass/treble instead of single intensity
        Err(anyhow::anyhow!(
            "Equalizer uses bass/treble levels, not single intensity"
        ))
    }

    fn intensity(&self) -> u8 {
        // Return average of bass and treble as a rough intensity indicator
        let avg = (self.bass.abs() + self.treble.abs()) as u8 / 2;
        avg.min(10)
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        debug!("Equalizer enabled: {}", enabled);
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for EqualizerEffect {
    fn default() -> Self {
        Self::new()
    }
}
