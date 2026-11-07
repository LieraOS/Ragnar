//! Public backend handle used by the GUI.
//!
//! Owns the command sender into the PipeWire worker thread and the
//! receiver for state events. The GUI drains `try_recv_event()` on a
//! glib timer; commands are fire-and-forget (`send` wakes the worker
//! loop through PipeWire's own channel mechanism).

use std::sync::mpsc;

use anyhow::Result;
use pipewire::channel::Sender;
use tracing::warn;

use super::types::{BackendCommand, BackendEvent, Direction, FxEffect, MicEffect, VoicePreset};
use super::worker;

/// Handle to the running PipeWire backend.
pub struct PwBackend {
    cmd_tx: Sender<BackendCommand>,
    event_rx: mpsc::Receiver<BackendEvent>,
}

impl PwBackend {
    /// Spawn the PipeWire worker thread.
    ///
    /// Always returns a handle once the thread is up; connection
    /// failures are reported asynchronously via `BackendEvent::BackendLost`.
    pub fn start() -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel();
        let cmd_tx = worker::spawn(event_tx)?;
        Ok(Self { cmd_tx, event_rx })
    }

    /// Set endpoint volume (linear gain; 1.0 = 100 %, max 1.5 with boost).
    pub fn set_volume(&self, id: u32, volume: f32) {
        self.send(BackendCommand::SetVolume { id, volume });
    }

    /// Set endpoint mute state.
    pub fn set_mute(&self, id: u32, muted: bool) {
        self.send(BackendCommand::SetMute { id, muted });
    }

    /// Make the named endpoint the session default for its direction.
    pub fn set_default(&self, name: &str, direction: Direction) {
        self.send(BackendCommand::SetDefault {
            name: name.to_string(),
            direction,
        });
    }

    /// Enable/disable the built-in effects chain (filter-chain modules
    /// are loaded/unloaded on the worker thread).
    pub fn set_fx_enabled(&self, enabled: bool) {
        self.send(BackendCommand::SetFxEnabled(enabled));
    }

    /// Live-update an effect control (slider units, see `FxEffect`).
    pub fn set_effect_value(&self, effect: FxEffect, value: f32) {
        self.send(BackendCommand::SetEffectValue { effect, value });
    }

    /// Enable/disable the microphone effects chain — only meaningful in
    /// the owning process (the `ragnar-skald-fx` daemon); the GUI drives
    /// it indirectly via the settings file.
    pub fn set_mic_fx_enabled(&self, enabled: bool) {
        self.send(BackendCommand::SetMicFxEnabled(enabled));
    }

    /// Change the active voice preset (rebuilds the voice stage).
    pub fn set_voice_preset(&self, preset: VoicePreset) {
        self.send(BackendCommand::SetVoicePreset(preset));
    }

    /// Toggle the noise-gate stage of the mic chain.
    pub fn set_mic_gate(&self, enabled: bool) {
        self.send(BackendCommand::SetMicGate(enabled));
    }

    /// Live-update a mic effect control (slider units, see `MicEffect`).
    pub fn set_mic_effect_value(&self, effect: MicEffect, value: f32) {
        self.send(BackendCommand::SetMicEffectValue { effect, value });
    }

    /// Loop the processed microphone to the default sink so the user
    /// can hear the configured effects ("Anhören" test toggle).
    pub fn set_mic_monitor(&self, enabled: bool) {
        self.send(BackendCommand::SetMicMonitor(enabled));
    }

    /// Attach the mic level meter to a source node (`None` removes it).
    pub fn set_meter_target(&self, id: Option<u32>) {
        self.send(BackendCommand::SetMeterTarget(id));
    }

    /// Non-blocking receive of backend events — drain once per timer tick.
    pub fn try_recv_event(&self) -> Result<BackendEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    fn send(&self, cmd: BackendCommand) {
        if self.cmd_tx.send(cmd).is_err() {
            warn!("PipeWire command channel closed — backend is gone");
        }
    }
}

impl Drop for PwBackend {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(BackendCommand::Quit);
    }
}
