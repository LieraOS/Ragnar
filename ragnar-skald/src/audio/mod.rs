// Copyright (C) Jörg Schröder
//! Audio Module
//!
//! Audio effects and processing implementation.

pub mod balance;
pub mod config;
pub mod effects;

pub use balance::BalanceControl;
pub use config::AudioConfig;
pub use effects::{AudioEffect, EchoEffect, EffectType, EqualizerEffect, ReverbEffect};
