//! Filter-chain lifecycle and runtime control for the [`Tracker`].
//!
//! Split out of `tracker.rs` to keep each file focused and under the
//! project size limit. The methods here are `pub(super)` implementation
//! details of the tracker; the public surface stays `BackendCommand`
//! (`SetFxEnabled`, `SetEffectValue`).

use libspa::param::ParamType;
use libspa::pod::Pod;
use tracing::{debug, warn};

use super::fxchain;
use super::ir;
use super::props;
use super::tracker::Tracker;
use super::types::{AudioEndpoint, BackendEvent, Direction, FxEffect};

impl Tracker {
    /// Register an fx capture node and decide whether it becomes a
    /// user-visible endpoint.
    ///
    /// All `*.in` captures are recorded in `fx_capture_ids` so runtime
    /// controls can reach them; only the *chain entry* module's sink is
    /// exposed as an endpoint (visible as "Skald Effekte" in the
    /// device list). `None` → internal stage, keep the proxy bound but
    /// do not surface it.
    pub(super) fn classify_fx(
        &mut self,
        id: u32,
        name: &str,
        direction: Direction,
        description: &str,
    ) -> Option<AudioEndpoint> {
        let effect = fxchain::effect_for_capture(name)?;
        let first = self.fx_capture_ids.is_empty();
        self.fx_capture_ids.insert(effect, id);
        if first {
            // Presence-driven "enabled" state: works whether this
            // process or the background daemon owns the modules.
            let _ = self.event_tx.send(BackendEvent::FxStateChanged(true));
        }
        debug!("Fx capture bound: {name} (id {id})");

        // Replay a control value that arrived before this node existed
        // (e.g. `restore()` pushes slider state right after enabling).
        if let Some(&value) = self.fx_values.get(&effect) {
            self.push_control(id, effect, value);
        }

        let is_entry = Some(effect) == fxchain::chain_order().first().copied();
        is_entry.then(|| AudioEndpoint {
            id,
            name: name.to_string(),
            description: description.to_string(),
            direction,
            volume: 1.0,
            channel_volumes: Vec::new(),
            muted: false,
            is_default: false,
        })
    }

    /// Load the whole effect chain and route the default output
    /// through it. Each module's `playback.props.target.object` points
    /// at the next module's capture node; the last module has no
    /// target so playback lands on the session default sink.
    pub(super) fn enable_fx(&mut self) {
        if !self.fx_modules.is_empty() {
            return; // already enabled
        }
        let ir_path = match ir::impulse_response_path() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                warn!("No reverb IR available: {e:#} — reverb skipped");
                String::new()
            }
        };
        let chain = fxchain::chain_order();
        // Effects that cannot load (missing IR) are skipped and the
        // following module becomes the link target instead.
        let usable: Vec<FxEffect> = chain
            .iter()
            .copied()
            .filter(|e| !(matches!(e, FxEffect::Reverb) && ir_path.is_empty()))
            .collect();
        // Load in reverse chain order so each module's `target.object`
        // (the next module's capture node) already exists when the
        // session manager links its playback stream — otherwise the
        // link falls back to whatever is the default at that moment.
        let mut next_capture: Option<String> = None; // last module: no target
        for effect in usable.iter().rev() {
            match fxchain::load_module(&self.context, *effect, next_capture.as_deref(), &ir_path) {
                Some(ptr) => {
                    self.fx_modules.insert(*effect, ptr);
                    next_capture = Some(fxchain::capture_node_name(*effect));
                }
                None => warn!("Effect {effect:?} failed to load"),
            }
        }
        if self.fx_modules.is_empty() {
            warn!("No effect modules loaded — effects stay disabled");
            let _ = self.event_tx.send(BackendEvent::FxStateChanged(false));
            return;
        }

        // Route audio through the chain: remember the real default so
        // `disable_fx` can restore it, then point the session at the
        // entry capture sink.
        self.prev_default_sink = self.default_name(Direction::Output).map(String::from);
        // Volume continuity: remember the real sink's level so the
        // chain entry can inherit it — WirePlumber would otherwise
        // restore a stale (possibly boosted) value onto the endpoint
        // and clip every stream it feeds.
        self.pending_fx_volume = self
            .endpoints
            .values()
            .find(|e| Some(&e.name) == self.prev_default_sink.as_ref())
            .map(|e| e.volume);
        let entry =
            fxchain::capture_node_name(*fxchain::chain_order().first().unwrap_or(&FxEffect::Bass));
        self.set_default(&entry, Direction::Output);
        let _ = self.event_tx.send(BackendEvent::FxStateChanged(true));
    }

    /// Unload all effect modules and restore the previous default sink.
    pub(super) fn disable_fx(&mut self) {
        if let Some(name) = self.prev_default_sink.take() {
            self.set_default(&name, Direction::Output);
        }
        // WirePlumber inherits the boosted fx-entry volume onto the
        // streams linked through the chain. Once they relink to the
        // real sink, anything >1.0 multiplies the device volume and
        // hard-clips — clamp boosted streams back to unity. Streams
        // at or below 100 % keep their level untouched.
        let boosted: Vec<u32> = self
            .streams
            .values()
            .filter(|s| s.volume > 1.0)
            .map(|s| s.id)
            .collect();
        for id in boosted {
            debug!("Clamping stream {id} back to 100 % after fx disable");
            self.set_volume(id, 1.0);
        }
        // Only clear capture ids when WE destroyed the modules — in
        // the GUI process `fx_modules` is empty (the daemon owns the
        // chain) and the ids must keep pointing at its live nodes.
        let owned = !self.fx_modules.is_empty();
        for (effect, ptr) in self.fx_modules.drain() {
            // SAFETY: every pointer came from `fxchain::load_module` on
            // this same context and is destroyed exactly once here.
            unsafe { fxchain::destroy_module(ptr) };
            debug!("Unloaded effect module {effect:?}");
        }
        if owned {
            // Dead server-side resources — a set_param before the
            // removal events would hit "unknown resource" errors.
            self.fx_capture_ids.clear();
        }
        let _ = self.event_tx.send(BackendEvent::FxStateChanged(false));
    }

    /// Destroy any modules still loaded when the worker shuts down.
    pub(super) fn cleanup_fx(&mut self) {
        for (effect, ptr) in self.fx_modules.drain() {
            // SAFETY: pointer from `fxchain::load_module`, destroyed once.
            unsafe { fxchain::destroy_module(ptr) };
            debug!("Cleaned up effect module {effect:?}");
        }
    }

    /// Push a live control value to an effect's capture node via a
    /// `Props`/`params` pod — this is what PipeWire's filter-graph
    /// applies to the first node of the module's graph. Values are
    /// remembered in `fx_values` so they can be replayed if the capture
    /// node binds later than the command arrives.
    pub(super) fn set_effect_value(&mut self, effect: FxEffect, value: f32) {
        self.fx_values.insert(effect, value);
        let Some(&id) = self.fx_capture_ids.get(&effect) else {
            debug!("Effect {effect:?} not loaded — value queued");
            return;
        };
        self.push_control(id, effect, value);
    }

    /// Build the `params` pod for `effect` and write it to node `id`.
    fn push_control(&self, id: u32, effect: FxEffect, value: f32) {
        let (control, scale) = effect.control();
        match props::build_control_pod(&[(control, value * scale)]) {
            Ok(bytes) => {
                if let Some(pod) = Pod::from_bytes(&bytes) {
                    if let Some(node) = self.node_proxy(id) {
                        node.set_param(ParamType::Props, 0, pod);
                    }
                }
            }
            Err(e) => warn!("Failed to build control pod for {effect:?}: {e}"),
        }
    }
}
