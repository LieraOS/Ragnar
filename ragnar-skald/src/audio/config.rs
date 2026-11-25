// Copyright (C) Jörg Schröder
//! Audio Configuration
//!
//! Audio configuration management.

use crate::audio::balance::BalanceControl;
use crate::audio::effects::{EchoEffect, EqualizerEffect, ReverbEffect};
use crate::pipewire::types::VoicePreset;
use serde::{Deserialize, Serialize};

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Reverb effect
    pub reverb: ReverbEffect,
    /// Echo effect
    pub echo: EchoEffect,
    /// Equalizer effect
    pub equalizer: EqualizerEffect,
    /// Balance control
    pub balance: BalanceControl,
    /// Master switch for the built-in filter-chain effects. Defaults
    /// to off; `serde(default)` keeps pre-existing config files valid.
    #[serde(default)]
    pub fx_enabled: bool,
    /// Master switch for the microphone effects chain (virtual source
    /// `skald.mic.out` becomes the session default input).
    #[serde(default)]
    pub mic_fx_enabled: bool,
    /// Active voice transformation preset.
    #[serde(default)]
    pub mic_preset: VoicePreset,
    /// Whether the noise-gate (denoise) stage is in the mic chain.
    #[serde(default)]
    pub mic_gate: bool,
    /// Noise-gate threshold slider, 0..10 → -55..-15 dB.
    /// Defaults to mid-range for config files written before the
    /// field existed (`serde(default)` alone would give 0.0).
    #[serde(default = "default_gate_level")]
    pub mic_gate_level: f32,
    /// Microphone echo slider, 0..10 → wet gain 0..0.5.
    #[serde(default)]
    pub mic_echo_level: f32,
    /// Voice pitch in semitones — the absolute value applied to the
    /// shifter; the GUI snaps it to the preset default on preset
    /// changes so it always reflects the user's last manual tuning.
    /// Defaults to the Female base (the most common target) for
    /// config files written before the field existed.
    #[serde(default = "default_voice_pitch")]
    pub mic_voice_pitch: f32,
}

/// Serde default for `mic_voice_pitch` (matches `AudioConfig::default`).
fn default_voice_pitch() -> f32 {
    6.0
}

/// Serde default for `mic_gate_level` (matches `AudioConfig::default`).
fn default_gate_level() -> f32 {
    5.0
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            reverb: ReverbEffect::new(),
            echo: EchoEffect::new(),
            equalizer: EqualizerEffect::new(),
            balance: BalanceControl::new(),
            fx_enabled: false,
            mic_fx_enabled: false,
            mic_preset: VoicePreset::Off,
            mic_gate: false,
            mic_gate_level: 5.0,
            mic_echo_level: 0.0,
            mic_voice_pitch: 6.0,
        }
    }
}

impl AudioConfig {
    /// Create a new audio configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all effects to default
    pub fn reset_effects(&mut self) {
        self.reverb = ReverbEffect::new();
        self.echo = EchoEffect::new();
        self.equalizer = EqualizerEffect::new();
        self.balance = BalanceControl::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings files written before the mic-fx schema must still load
    /// — every new field carries `serde(default)`. Simulated by
    /// serializing a current config and deleting the new keys, which
    /// reproduces exactly what an older version would have written.
    #[test]
    fn legacy_settings_without_mic_fields_load() {
        let mut legacy = serde_json::to_value(AudioConfig::default()).expect("serialize");
        let map = legacy.as_object_mut().expect("config is an object");
        for key in [
            "fx_enabled",
            "mic_fx_enabled",
            "mic_preset",
            "mic_gate",
            "mic_gate_level",
            "mic_echo_level",
        ] {
            map.remove(key);
        }
        let cfg: AudioConfig =
            serde_json::from_value(legacy).expect("legacy config must deserialize");
        assert!(!cfg.fx_enabled);
        assert!(!cfg.mic_fx_enabled);
        assert_eq!(cfg.mic_preset, VoicePreset::Off);
        assert!(!cfg.mic_gate);
        // Mid-range default from the serde default fn, not 0.0.
        assert_eq!(cfg.mic_gate_level, 5.0);
        assert_eq!(cfg.mic_echo_level, 0.0);
    }

    /// Mic fields round-trip through JSON.
    #[test]
    fn mic_fields_roundtrip() {
        let cfg = AudioConfig {
            mic_fx_enabled: true,
            mic_preset: VoicePreset::Robot,
            mic_gate: true,
            mic_gate_level: 7.5,
            mic_echo_level: 3.0,
            ..AudioConfig::default()
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: AudioConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(back.mic_fx_enabled);
        assert_eq!(back.mic_preset, VoicePreset::Robot);
        assert!(back.mic_gate);
        assert_eq!(back.mic_gate_level, 7.5);
        assert_eq!(back.mic_echo_level, 3.0);
    }
}
