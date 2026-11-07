//! PipeWire backend module.
//!
//! A dedicated worker thread owns the PipeWire main loop, context and
//! registry. The GTK frontend communicates exclusively through
//! [`types::BackendCommand`]s (sent via [`backend::PwBackend`]) and
//! receives [`types::BackendEvent`]s back.
//!
//! Layout:
//! - [`backend`]: public handle for the GUI
//! - [`worker`]: thread setup, main loop, core/registry listeners
//! - [`tracker`]: bound node proxies, endpoint state, registry tracking
//! - [`tracker_meta`]: session defaults via the "default" metadata object
//! - [`props`]: SPA `Props` pod serialize/parse for volume and mute
//! - [`fxchain`]: built-in output effects (module load/linking)
//! - [`tracker_fx`]: output effect lifecycle and runtime control
//! - [`tracker_props`]: endpoint Props handling (volume/mute read/write)
//! - [`micfx`]: microphone effects (capture-side filter-chain stages)
//! - [`tracker_mic`]: mic chain lifecycle, monitor loopback, meter
//! - [`meter`]: live mic level capture stream (peak → `MicLevel`)
//! - [`ir`]: synthesized reverb impulse response for the convolver
//! - [`types`]: command/event/data contract between threads

pub mod backend;
pub mod fxchain;
pub mod ir;
pub mod meter;
pub mod micfx;
pub mod props;
pub mod tracker;
pub mod tracker_fx;
pub mod tracker_meta;
pub mod tracker_mic;
mod tracker_props;
pub mod types;
pub mod voice_fx;
pub mod worker;

pub use backend::PwBackend;
pub use types::{
    AudioEndpoint, BackendCommand, BackendEvent, Direction, FxEffect, LadspaPlugin, MicEffect,
    VoicePreset, MAX_VOLUME,
};
