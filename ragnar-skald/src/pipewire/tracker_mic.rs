//! Microphone chain lifecycle, preview monitor and level meter.
//!
//! Split out of `tracker.rs` like `tracker_fx.rs`. The mic chain is the
//! input-direction mirror of the output chain: stage modules capture
//! from the upstream source (`target.object` on `capture.props`) and
//! present an `Audio/Source` on their playback side. The final stage
//! exposes the stable name `skald.mic.out`, which is made the
//! session default input so every application records the processed
//! signal until the chain is disabled.

use libspa::param::ParamType;
use libspa::pod::Pod;
use tracing::{debug, warn};

use super::fxchain;
use super::meter::MicMeter;
use super::micfx;
use super::props;
use super::tracker::Tracker;
use super::types::{BackendEvent, Direction, MicEffect};

impl Tracker {
    /// Register a mic-chain capture node (`skald.micfx.*.in`) for
    /// runtime control. Mic-chain nodes never become endpoints — the
    /// user-facing handle is the final `skald.mic.out` source.
    pub(super) fn classify_micfx(&mut self, id: u32, name: &str) {
        let Some(effect) = micfx::effect_for_capture(name) else {
            return;
        };
        self.mic_capture_ids.insert(effect, id);
        debug!("Mic fx capture bound: {name} (id {id})");

        // Replay a control value that arrived before this node existed.
        if let Some(&value) = self.mic_values.get(&effect) {
            self.push_mic_control(id, effect, value);
        }
    }

    /// `true` while any mic processing is wanted: either the effects
    /// The mic chain is permanent infrastructure, not an option:
    /// `skald.mic.out` is the session's default input at all
    /// times so applications only ever see the software-gain virtual
    /// source — never the raw ADC whose volume mapping slams the
    /// hardware gain to clipping. Effect stages toggle inside the
    /// chain; the chain itself never unloads.
    pub(super) fn mic_wanted(&self) -> bool {
        true
    }

    /// Hardware capture level the real source gets pinned to while
    /// the chain runs — a LINEAR gain of ~0.22 lands near +15 dB on
    /// typical ACP mixers, loud enough for mixer-fed mics while
    /// staying well below the +30 dB clip zone (which begins around
    /// 0.3). All user-visible loudness then happens in software on
    /// `skald.mic.out` instead of the ADC. `set_volume` expects the
    /// cubic display convention, so the constant holds cbrt(0.22).
    const SAFE_CAPTURE_VOLUME: f32 = 0.604;

    /// Enable the chain if wanted and possible; defers via
    /// `mic_pending` while the registry is still incomplete (no
    /// metadata object or no real source enumerated yet).
    pub(super) fn request_mic_chain(&mut self) {
        if !self.mic_modules.is_empty() {
            return;
        }
        if self.default_metadata.is_none() || self.real_source_name().is_none() {
            self.mic_pending = true;
            return;
        }
        self.enable_mic_fx();
    }

    /// Stages actually loaded for the mic chain, in signal order.
    /// Denoise is gated only by its own toggle + plugin availability;
    /// voice and echo belong to the effects master switch. An
    /// unavailable plugin produces a passthrough module rather than a
    /// broken chain.
    fn mic_stages(&self) -> Vec<MicEffect> {
        let stages: Vec<MicEffect> = micfx::CHAIN_ORDER
            .iter()
            .copied()
            .filter(|e| match e {
                MicEffect::Denoise => self.mic_gate && micfx::denoise_available(),
                MicEffect::Voice => {
                    self.mic_enabled && self.mic_preset != super::types::VoicePreset::Off
                }
                MicEffect::Echo => self.mic_enabled,
                MicEffect::Pass => false,
            })
            .collect();
        // No stage wanted → transparent passthrough keeps the virtual
        // mic alive so the software-gain default never disappears.
        if stages.is_empty() {
            vec![MicEffect::Pass]
        } else {
            stages
        }
    }

    /// The real microphone to feed the chain from: the configured
    /// default source, unless that is already our own virtual mic
    /// (e.g. on a chain rebuild) — then the remembered pre-chain
    /// source wins. Falls back to the first non-Skald source.
    pub(super) fn real_source_name(&self) -> Option<String> {
        if let Some(d) = self.default_names.get(&Direction::Input) {
            if !d.starts_with("skald.") {
                return Some(d.clone());
            }
        }
        if let Some(p) = &self.prev_default_source {
            return Some(p.clone());
        }
        self.endpoints
            .values()
            .filter(|e| e.direction == Direction::Input && !e.name.starts_with("skald."))
            .min_by_key(|e| e.id)
            .map(|e| e.name.clone())
    }

    /// Load the mic chain and point the session default input at it.
    /// Stages load in FORWARD order — each capture stream's
    /// `target.object` is the previous stage's playback node, which
    /// must already exist when the session manager links the stream.
    pub(super) fn enable_mic_fx(&mut self) {
        if !self.mic_modules.is_empty() {
            return; // already enabled
        }
        let Some(mut upstream) = self.real_source_name() else {
            warn!("No real input source — mic effects not loaded");
            let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(false));
            return;
        };
        let stages = self.mic_stages();
        if stages.is_empty() {
            warn!("Mic effects enabled but no stage is usable");
            let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(false));
            return;
        }
        let pitch = micfx::pitch_plugin();
        if self.mic_preset.required_plugin().is_some()
            && pitch.is_none()
            && self.mic_preset != super::types::VoicePreset::Robot
        {
            warn!("No pitch plugin found — voice preset runs passthrough");
        }

        // Remember the real default source before we take over so
        // `disable_mic_fx` can restore it.
        self.prev_default_source = Some(upstream.clone());

        for (i, stage) in stages.iter().enumerate() {
            let is_last = i + 1 == stages.len();
            let args = micfx::module_args(*stage, &upstream, is_last, self.mic_preset, pitch);
            match fxchain::load_module_raw(&self.context, "libpipewire-module-filter-chain", &args)
            {
                Some(ptr) => {
                    self.mic_modules.insert(*stage, ptr);
                    upstream = if is_last {
                        micfx::MIC_OUT_NODE.to_string()
                    } else {
                        format!("{}{}.out", micfx::MICFX_NODE_PREFIX, stage.key())
                    };
                }
                None => warn!("Mic effect {stage:?} failed to load"),
            }
        }
        if self.mic_modules.is_empty() {
            warn!("No mic effect modules loaded — mic effects stay disabled");
            self.prev_default_source = None;
            let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(false));
            return;
        }

        self.set_default(micfx::MIC_OUT_NODE, Direction::Input);
        // Pin the real source's hardware gain: PW volume on ACP
        // sources maps straight onto the ALSA ADC gain — ~0.3 already
        // means ~+28 dB and hard-clips a mixer-fed line input. Apps
        // (Discord, the GUI slider) adjust the VIRTUAL source's
        // software volume instead; the hardware stays parked at a
        // sane capture level. `prev_source_volume` restores the old
        // value when the chain unloads.
        let real = self.prev_default_source.clone().unwrap_or_default();
        if let Some((id, vol)) = self
            .endpoints
            .iter()
            .find(|(_, e)| e.direction == Direction::Input && e.name == real)
            .map(|(id, e)| (*id, e.volume))
        {
            self.prev_source_volume = Some((id, vol));
            self.set_volume(id, Self::SAFE_CAPTURE_VOLUME);
        }
        let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(true));
    }

    /// Unload the mic chain. `restore_default` puts the remembered real
    /// source back as session default — skipped on internal rebuilds
    /// where the chain immediately takes over again.
    pub(super) fn disable_mic_fx(&mut self, restore_default: bool) {
        if restore_default {
            if let Some(name) = self.prev_default_source.take() {
                self.set_default(&name, Direction::Input);
            }
        }
        // Release the pinned hardware gain on every unload — also
        // on internal rebuilds, where a different source may be
        // about to take the pin.
        if let Some((id, vol)) = self.prev_source_volume.take() {
            self.set_volume(id, vol);
        }
        // Only when WE destroyed modules do our capture ids die — in
        // the GUI process `mic_modules` is empty (the daemon owns the
        // chain) and `mic_capture_ids` must keep pointing at the
        // daemon's live nodes.
        let owned = !self.mic_modules.is_empty();
        for (effect, ptr) in self.mic_modules.drain() {
            // SAFETY: every pointer came from `fxchain::load_module_raw`
            // on this same context and is destroyed exactly once here.
            unsafe { fxchain::destroy_module(ptr) };
            debug!("Unloaded mic effect module {effect:?}");
        }
        if owned {
            // The nodes are gone server-side even before the removal
            // events arrive — a set_param in between would hit a dead
            // resource ("unknown resource" core error).
            self.mic_capture_ids.clear();
        }
    }

    /// Rebuild the mic chain in place (preset/gate changes alter graph
    /// membership — a running reload is the clean way to rewire).
    pub(super) fn reload_mic_chain(&mut self) {
        self.disable_mic_fx(false);
        self.enable_mic_fx();
    }

    /// Destroy any mic-chain modules and the monitor loopback when the
    /// worker shuts down.
    pub(super) fn cleanup_mic_fx(&mut self) {
        self.disable_mic_fx(false);
        self.set_mic_monitor(false);
        self.mic_meter = None;
        self.meter_target = None;
    }

    /// Push a live control value to a mic stage's capture node via a
    /// `Props`/`params` pod (same mechanism as the output chain).
    pub(super) fn set_mic_effect_value(&mut self, effect: MicEffect, value: f32) {
        self.mic_values.insert(effect, value);
        let Some(&id) = self.mic_capture_ids.get(&effect) else {
            debug!("Mic effect {effect:?} not loaded — value queued");
            return;
        };
        self.push_mic_control(id, effect, value);
    }

    /// Build the `params` pod for a mic stage and write it to node `id`.
    fn push_mic_control(&self, id: u32, effect: MicEffect, value: f32) {
        let Some((control, scale, offset)) = micfx::effect_control(effect, self.mic_preset) else {
            return; // stages without a runtime control (voice)
        };
        match props::build_control_pod(&[(control, offset + value * scale)]) {
            Ok(bytes) => {
                if let Some(pod) = Pod::from_bytes(&bytes) {
                    if let Some(node) = self.node_proxy(id) {
                        node.set_param(ParamType::Props, 0, pod);
                    }
                }
            }
            Err(e) => warn!("Failed to build mic control pod for {effect:?}: {e}"),
        }
    }

    /// Best source for the preview monitor: the processed virtual mic
    /// whenever it exists in the registry (a background daemon owns the
    /// chain — `mic_modules` is empty in the GUI process), else the
    /// real microphone so monitoring still works with effects off.
    fn mic_monitor_source(&self) -> Option<String> {
        self.endpoints
            .values()
            .any(|e| e.direction == Direction::Input && e.name == micfx::MIC_OUT_NODE)
            .then(|| micfx::MIC_OUT_NODE.to_string())
            .or_else(|| self.real_source_name())
    }

    /// Load/unload the `libpipewire-module-loopback` monitor that lets
    /// the user hear their (processed) microphone on the default sink.
    /// Owned by the GUI process — it dies with the window, which is a
    /// feature: a forgotten monitor path is a feedback hazard.
    pub(super) fn set_mic_monitor(&mut self, on: bool) {
        if on == self.mic_monitor.is_some() {
            return;
        }
        if !on {
            self.unload_mic_monitor();
            return;
        }
        let Some(mic) = self.mic_monitor_source() else {
            warn!("No input source for mic monitor");
            return;
        };
        self.load_mic_monitor(&mic);
    }

    /// Called when the virtual mic appears or disappears: if the
    /// monitor is active and its capture target is no longer the best
    /// source, reload the loopback onto the better one. This keeps the
    /// preview on the processed signal across chain rebuilds.
    pub(super) fn retarget_mic_monitor(&mut self) {
        if self.mic_monitor.is_none() {
            return;
        }
        let want = self.mic_monitor_source();
        if want == self.mic_monitor_target {
            return;
        }
        self.unload_mic_monitor();
        if let Some(mic) = want {
            self.load_mic_monitor(&mic);
        }
    }

    /// Destroy the monitor module if loaded and forget its target.
    fn unload_mic_monitor(&mut self) {
        if let Some(ptr) = self.mic_monitor.take() {
            // SAFETY: pointer from `load_module_raw` on this context.
            unsafe { fxchain::destroy_module(ptr) };
            debug!("Mic monitor loopback unloaded");
        }
        self.mic_monitor_target = None;
    }

    /// Instantiate the loopback capturing `mic`; playback follows the
    /// current default sink (through the output chain when enabled —
    /// the user hears the full pipeline).
    fn load_mic_monitor(&mut self, mic: &str) {
        // `target.object` on the playback stream is only emitted when a
        // default sink is known — an empty string would pin the link to
        // nothing instead of letting the session manager pick.
        let playback_target = self
            .default_names
            .get(&Direction::Output)
            .map(|s| format!("target.object = \"{s}\""))
            .unwrap_or_default();

        let args = format!(
            "{{ node.description = \"Skald Mikrofon-Test\" \
                capture.props = {{ \
                    node.name = \"skald.listen.in\" \
                    node.description = \"Skald Mikrofon-Test\" \
                    target.object = \"{mic}\" \
                }} \
                playback.props = {{ \
                    node.name = \"skald.listen.out\" \
                    node.description = \"Skald Mikrofon-Test\" \
                    {playback_target} \
                }} \
            }}"
        );
        match fxchain::load_module_raw(&self.context, "libpipewire-module-loopback", &args) {
            Some(ptr) => {
                self.mic_monitor = Some(ptr);
                self.mic_monitor_target = Some(mic.to_string());
                debug!("Mic monitor loopback capturing {mic}");
            }
            None => warn!("Failed to load loopback module for mic monitor"),
        }
    }

    /// Point the level meter at source node `id` — `None` removes it.
    /// The meter lives on the GUI's worker; the daemon never creates
    /// one. Re-targeting an unchanged id is a no-op so endpoint-list
    /// rebuilds do not churn the stream.
    pub(super) fn set_meter_target(&mut self, id: Option<u32>) {
        if self.meter_target == id {
            return;
        }
        self.meter_target = id;
        self.mic_meter = id.and_then(|t| MicMeter::new(&self.core, Some(t), self.event_tx.clone()));
    }
}
