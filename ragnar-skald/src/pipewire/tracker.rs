// Copyright (C) Jörg Schröder
//! Registry tracker: owns bound node proxies, endpoint state and the
//! "default" metadata object that selects session defaults.
//!
//! Runs entirely on the PipeWire worker thread. All mutation happens
//! inside callbacks dispatched by the PipeWire main loop, so plain
//! `Rc<RefCell<..>>` interior mutability is sufficient.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::mpsc;

use libspa::param::ParamType;
use libspa::utils::dict::DictRef;
use pipewire::context::Context;
use pipewire::core::Core;
use pipewire::metadata::{Metadata, MetadataListener};
use pipewire::node::{Node, NodeListener};
use pipewire::registry::{GlobalObject, Registry};
use pipewire::sys as pw_sys;
use pipewire::types::ObjectType;
use tracing::{debug, warn};

use super::fxchain;
use super::meter::MicMeter;
use super::micfx;
use super::props;
use super::types::{
    AppStream, AudioEndpoint, BackendCommand, BackendEvent, Direction, FxEffect, MicEffect,
    VoicePreset,
};

/// Key on metadata globals that identifies WirePlumber's "default" store.
const DEFAULT_METADATA_NAME: &str = "default";

/// Keeps a bound node proxy and its listener alive for the node's
/// lifetime. Dropping it destroys the proxy and unregisters callbacks.
pub(super) struct NodeHandle {
    pub(super) proxy: Node,
    pub(super) _listener: NodeListener,
}

/// Tracks all PipeWire objects relevant to audio endpoint control.
pub struct Tracker {
    /// Sink for events pushed to the GTK thread.
    pub(super) event_tx: mpsc::Sender<BackendEvent>,
    /// Known endpoints by registry id.
    pub(super) endpoints: HashMap<u32, AudioEndpoint>,
    /// Bound node proxies keyed by registry id.
    pub(super) nodes: HashMap<u32, NodeHandle>,
    /// The bound "default" metadata object (WirePlumber).
    pub(super) default_metadata: Option<(Metadata, MetadataListener)>,
    /// Configured default node name per direction.
    pub(super) default_names: HashMap<Direction, String>,
    /// Weak self-reference so bound listeners can reach the tracker.
    pub(super) me: Weak<RefCell<Tracker>>,
    /// Client context for loading filter-chain modules.
    pub(super) context: Context,
    /// Connected core — needed to create the mic meter stream.
    pub(super) core: Core,
    /// Loaded output effect modules (raw pointers owned by our context).
    pub(super) fx_modules: HashMap<FxEffect, *mut pw_sys::pw_impl_module>,
    /// Registry id of each effect's capture node (`skald.fx.*.in`).
    pub(super) fx_capture_ids: HashMap<FxEffect, u32>,
    /// The real sink that was default before effects were enabled.
    pub(super) prev_default_sink: Option<String>,
    /// `true` while a SetFxEnabled request waits for the "default"
    /// metadata object to bind (it may arrive after the command).
    pub(super) fx_pending: bool,
    /// Last requested control value per effect — capture nodes appear
    /// asynchronously after module load, so values arriving early are
    /// replayed when the node binds.
    pub(super) fx_values: HashMap<FxEffect, f32>,
    /// Loaded microphone effect modules (raw pointers).
    pub(super) mic_modules: HashMap<MicEffect, *mut pw_sys::pw_impl_module>,
    /// Registry ids of mic-chain capture nodes (`skald.micfx.*.in`).
    pub(super) mic_capture_ids: HashMap<MicEffect, u32>,
    /// Queued mic-effect control values (same bind race as `fx_values`).
    pub(super) mic_values: HashMap<MicEffect, f32>,
    /// The real source that was default before the mic chain took over.
    pub(super) prev_default_source: Option<String>,
    /// The real source's volume before the chain pinned the hardware
    /// gain — restored on unload so the raw path isn't left quiet.
    pub(super) prev_source_volume: Option<(u32, f32)>,
    /// User intent: the mic effects master switch (voice preset + echo).
    /// The chain also runs for `mic_gate` alone — denoise works
    /// independently of the fun effects.
    pub(super) mic_enabled: bool,
    /// Currently configured voice preset (rebuilds the voice stage).
    pub(super) mic_preset: VoicePreset,
    /// Whether the noise-gate stage is part of the mic chain.
    pub(super) mic_gate: bool,
    /// `true` while a SetMicFxEnabled request waits for metadata.
    pub(super) mic_pending: bool,
    /// Microphone level meter stream (GUI-owned, dies with the app).
    pub(super) mic_meter: Option<MicMeter>,
    /// Loopback module for the "hear yourself" preview (GUI-owned).
    pub(super) mic_monitor: Option<*mut pw_sys::pw_impl_module>,
    /// The source node name the monitor loopback currently captures —
    /// re-resolved when the processed mic appears or disappears so the
    /// preview always follows the live chain (which a background
    /// daemon may own; `mic_modules` is empty in the GUI process).
    pub(super) mic_monitor_target: Option<String>,
    /// Live app playback streams (`Stream/Output/Audio`) keyed by id —
    /// rendered as dynamic per-app volume cards on the output page.
    pub(super) streams: HashMap<u32, AppStream>,
    /// Registry ids of internal `skald.fx.*.out` plumbing nodes.
    /// Their channelVolumes multiply through the chain — WirePlumber
    /// restores whatever value they last had, so >1.0 leftovers from
    /// an old boost state silently clip the whole output. They must
    /// be forced back to unity whenever a volume lands on them.
    pub(super) fx_plumbing: HashSet<u32>,
    /// Volume the fx entry endpoint must take on its first Props —
    /// WirePlumber restores whatever it cached last (e.g. a boosted
    /// 129 %), which multiplies on top of every stream and is the
    /// re-enable overdrive. We carry the previous sink's level over
    /// instead, so enabling the chain keeps the same loudness.
    pub(super) pending_fx_volume: Option<f32>,
    /// Source id the meter is currently attached to — repeated
    /// `SetMeterTarget` commands with the same id are a no-op.
    pub(super) meter_target: Option<u32>,
}

impl Tracker {
    pub fn new(event_tx: mpsc::Sender<BackendEvent>, context: Context, core: Core) -> Self {
        Self {
            event_tx,
            endpoints: HashMap::new(),
            nodes: HashMap::new(),
            default_metadata: None,
            default_names: HashMap::new(),
            me: Weak::new(),
            context,
            core,
            fx_modules: HashMap::new(),
            fx_capture_ids: HashMap::new(),
            prev_default_sink: None,
            fx_pending: false,
            fx_values: HashMap::new(),
            mic_modules: HashMap::new(),
            mic_capture_ids: HashMap::new(),
            mic_values: HashMap::new(),
            prev_default_source: None,
            prev_source_volume: None,
            mic_enabled: false,
            mic_preset: VoicePreset::Off,
            mic_gate: false,
            mic_pending: false,
            mic_meter: None,
            mic_monitor: None,
            mic_monitor_target: None,
            meter_target: None,
            streams: HashMap::new(),
            fx_plumbing: HashSet::new(),
            pending_fx_volume: None,
        }
    }

    /// Must be called once after the tracker is wrapped in `Rc<RefCell>`;
    /// bound listeners use the weak pointer to reach back into it.
    pub fn set_self(&mut self, me: &Rc<RefCell<Tracker>>) {
        self.me = Rc::downgrade(me);
    }

    /// Registry `global` callback: inspect new objects and bind the ones
    /// relevant to us (audio sink/source nodes, the default metadata).
    pub fn add_global(&mut self, registry: &Rc<Registry>, obj: &GlobalObject<&DictRef>) {
        match obj.type_ {
            ObjectType::Node => self.add_node(registry, obj),
            ObjectType::Metadata => self.add_metadata(registry, obj),
            _ => {}
        }
    }

    /// Registry `global_remove` callback: drop proxy and state.
    ///
    /// Emits `FxStateChanged`/`MicFxStateChanged` when the last fx node
    /// disappears — the events reflect *presence in the registry*, so
    /// the GUI tracks chains owned by the background daemon too.
    pub fn remove_global(&mut self, id: u32) {
        self.nodes.remove(&id);
        let had_fx = !self.fx_capture_ids.is_empty();
        self.fx_capture_ids.retain(|_, v| *v != id);
        self.mic_capture_ids.retain(|_, v| *v != id);
        if had_fx && self.fx_capture_ids.is_empty() {
            let _ = self.event_tx.send(BackendEvent::FxStateChanged(false));
        }
        if self.streams.remove(&id).is_some() {
            debug!("Stream {id} removed");
            self.emit_streams();
        }
        if let Some(ep) = self.endpoints.remove(&id) {
            debug!("Endpoint {id} removed");
            if ep.name == micfx::MIC_OUT_NODE {
                let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(false));
                // A running preview falls back to the raw mic until
                // the chain comes back.
                self.retarget_mic_monitor();
            }
            self.emit_endpoints();
        }
    }

    /// Bind a node proxy and subscribe to its `Props` params.
    /// Returns `false` when the registry bind failed.
    fn bind_node(&mut self, registry: &Rc<Registry>, obj: &GlobalObject<&DictRef>) -> bool {
        let node: Node = match registry.bind(obj) {
            Ok(n) => n,
            Err(e) => {
                warn!("Failed to bind node {}: {e}", obj.id);
                return false;
            }
        };

        // The param listener needs a tracker handle to update state.
        let me = self.me.clone();
        let id = obj.id;
        let listener = node
            .add_listener_local()
            .param(move |_seq, _id, _index, _next, pod| {
                let (Some(tracker), Some(pod)) = (me.upgrade(), pod) else {
                    return;
                };
                if let Some(state) = props::parse_props_pod(pod) {
                    tracker.borrow_mut().apply_props(id, state);
                }
            })
            .register();
        node.subscribe_params(&[ParamType::Props]);
        // subscribe_params may never push the initial value on some
        // PipeWire versions — an explicit enum forces the Props dump
        // so the endpoint shows real volume/mute right away.
        node.enum_params(0, Some(ParamType::Props), 0, u32::MAX);

        self.nodes.insert(
            id,
            NodeHandle {
                proxy: node,
                _listener: listener,
            },
        );
        true
    }

    /// Bind a sink/source node, subscribe to its `Props` params and add
    /// the endpoint to the model.
    fn add_node(&mut self, registry: &Rc<Registry>, obj: &GlobalObject<&DictRef>) {
        let props = match obj.props {
            Some(p) => p,
            None => return,
        };

        let name = dict_get(props, "node.name").unwrap_or_default().to_string();

        // Internal mic-chain nodes are no user-facing endpoints:
        // `.in` capture streams get bound for runtime control only,
        // intermediate `.out` sources need no proxy at all. The final
        // `skald.mic.out` source does NOT match the prefix and
        // follows the normal endpoint path below.
        if micfx::is_micfx_node(&name) {
            if name.ends_with(".in") && self.bind_node(registry, obj) {
                self.classify_micfx(obj.id, &name);
            }
            return;
        }

        // Application playback streams are tracked separately —
        // the output page renders them as dynamic volume cards. Our
        // own filter-chain `.out` nodes also carry this class — they
        // are internal plumbing, never user-facing "apps".
        if dict_get(props, "media.class") == Some("Stream/Output/Audio") {
            let name = dict_get(props, "node.name").unwrap_or_default();
            if !name.starts_with("skald.") {
                self.add_stream(registry, obj);
            }
            return;
        }

        let direction = match dict_get(props, "media.class") {
            Some(c) if c == Direction::Output.media_class() => Direction::Output,
            Some(c) if c == Direction::Input.media_class() => Direction::Input,
            _ => return, // filters etc. are not endpoints
        };

        let description = dict_get(props, "node.description")
            .or_else(|| dict_get(props, "node.nick"))
            .unwrap_or(&name)
            .to_string();

        if !self.bind_node(registry, obj) {
            return;
        }
        let id = obj.id;

        // Our own filter-chain nodes get bound like normal endpoints
        // (needed for runtime control updates), but only the chain
        // entry capture appears in the device list — internal stages
        // stay hidden. `classify_fx` returns `None` for those.
        let ep = if fxchain::is_fx_node(&name) {
            match self.classify_fx(id, &name, direction, &description) {
                Some(ep) => ep,
                None => {
                    // Internal stage output — not an endpoint, but its
                    // volume must be pinned to unity (see fx_plumbing).
                    if name.ends_with(".out") {
                        self.fx_plumbing.insert(id);
                    }
                    return;
                }
            }
        } else {
            AudioEndpoint {
                id,
                name,
                description,
                direction,
                volume: 1.0,
                channel_volumes: Vec::new(),
                muted: false,
                // Recomputed by refresh_defaults() right below.
                is_default: false,
            }
        };
        debug!("Endpoint {id} added: {}", ep.description);
        let is_virtual_mic = ep.name == micfx::MIC_OUT_NODE;
        self.endpoints.insert(id, ep);
        if is_virtual_mic {
            let _ = self.event_tx.send(BackendEvent::MicFxStateChanged(true));
            // A running preview switches from the raw mic onto the
            // processed virtual mic.
            self.retarget_mic_monitor();
        }
        self.refresh_defaults();
        self.emit_endpoints();

        // A deferred mic-fx enable waits for a real source to appear —
        // registry enumeration may lag behind the metadata bind.
        if self.mic_pending && direction == Direction::Input && self.default_metadata.is_some() {
            self.mic_pending = false;
            self.enable_mic_fx();
        }
    }

    /// Track an app playback stream: bind it so `Props` params report
    /// its volume/mute, then add it to the stream list. The stream
    /// vanishes again when the app stops playing.
    fn add_stream(&mut self, registry: &Rc<Registry>, obj: &GlobalObject<&DictRef>) {
        let Some(props) = obj.props else { return };
        if !self.bind_node(registry, obj) {
            return;
        }
        let name = dict_get(props, "node.name").unwrap_or_default().to_string();
        // Electron apps (Discord, Steam, VSCode) report a generic
        // `application.name` like "Chromium" — the process binary or
        // node name usually identifies them better.
        let mut app_name = dict_get(props, "application.name")
            .unwrap_or("")
            .to_string();
        let generic = matches!(
            app_name.to_ascii_lowercase().as_str(),
            "chromium" | "chrome" | "electron" | "javascript"
        );
        if app_name.is_empty() || generic {
            let better = dict_get(props, "application.process.binary")
                .map(|b| b.rsplit('/').next().unwrap_or(b))
                .or_else(|| dict_get(props, "node.name"))
                .unwrap_or(&name);
            app_name = better.to_string();
        }
        if app_name.is_empty() {
            app_name = dict_get(props, "media.name")
                .or_else(|| dict_get(props, "node.description"))
                .unwrap_or(&name)
                .to_string();
        }
        self.streams.insert(
            obj.id,
            AppStream {
                id: obj.id,
                name,
                app_name: app_name.clone(),
                volume: 1.0,
                muted: false,
            },
        );
        debug!("Stream {} added: {app_name}", obj.id);
        self.emit_streams();
    }

    /// Push the current stream list to the GUI.
    pub(super) fn emit_streams(&self) {
        let mut list: Vec<AppStream> = self.streams.values().cloned().collect();
        list.sort_by_key(|s| s.id);
        let _ = self.event_tx.send(BackendEvent::StreamsChanged(list));
    }

    /// Bind the "default" metadata object so we can read and write the
    /// configured default endpoints (same mechanism `wpctl` uses).
    fn add_metadata(&mut self, registry: &Rc<Registry>, obj: &GlobalObject<&DictRef>) {
        let is_default_store = obj
            .props
            .and_then(|p| dict_get(p, "metadata.name"))
            .map(|n| n == DEFAULT_METADATA_NAME)
            .unwrap_or(false);
        if !is_default_store || self.default_metadata.is_some() {
            return;
        }

        let metadata: Metadata = match registry.bind(obj) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to bind default metadata: {e}");
                return;
            }
        };

        let me = self.me.clone();
        let listener = metadata
            .add_listener_local()
            .property(move |_subject, key, _type_, value| {
                if let Some(tracker) = me.upgrade() {
                    tracker.borrow_mut().apply_default_property(key, value);
                }
                0
            })
            .register();

        self.bind_default_metadata(metadata, listener);
    }

    /// Dispatch a command received from the GUI thread.
    pub fn dispatch_command(&mut self, cmd: BackendCommand) {
        match cmd {
            BackendCommand::SetVolume { id, volume } => self.set_volume(id, volume),
            BackendCommand::SetMute { id, muted } => self.set_mute(id, muted),
            BackendCommand::SetDefault { name, direction } => {
                // Split-state guard: while the output fx chain is
                // loaded (`prev_default_sink` set), a default switch
                // away from the fx entry must tear the chain down —
                // otherwise apps route around it while its output
                // link still feeds the sink, and the summed signal
                // clips hard. Covers external tools (wpctl,
                // pavucontrol), not just our GUI.
                if direction == Direction::Output
                    && self.prev_default_sink.is_some()
                    && name
                        != fxchain::capture_node_name(
                            *fxchain::chain_order().first().unwrap_or(&FxEffect::Bass),
                        )
                {
                    self.disable_fx();
                }
                self.set_default(&name, direction);
            }
            BackendCommand::SetFxEnabled(enabled) => {
                self.fx_pending = false;
                if enabled && self.default_metadata.is_none() {
                    // Default metadata has not appeared yet — retry
                    // when `add_metadata` binds it.
                    self.fx_pending = true;
                } else if enabled {
                    self.enable_fx();
                } else {
                    self.disable_fx();
                }
            }
            BackendCommand::SetEffectValue { effect, value } => {
                self.set_effect_value(effect, value)
            }
            BackendCommand::SetMicFxEnabled(enabled) => {
                let changed = self.mic_enabled != enabled;
                self.mic_enabled = enabled;
                if self.mic_wanted() {
                    if self.mic_modules.is_empty() {
                        self.request_mic_chain();
                    } else if changed {
                        // Stage membership changed (voice/echo join or
                        // leave while the gate keeps the chain alive).
                        self.reload_mic_chain();
                    }
                } else {
                    self.mic_pending = false;
                    self.disable_mic_fx(true);
                }
            }
            BackendCommand::SetVoicePreset(preset) => {
                // Preset changes alter the graph itself — rebuild the
                // chain so the new module wiring is picked up. Same
                // value → no rebuild (the daemon re-sends commands on
                // every settings write).
                if self.mic_preset != preset {
                    self.mic_preset = preset;
                    if !self.mic_modules.is_empty() {
                        self.reload_mic_chain();
                    }
                }
            }
            BackendCommand::SetMicGate(enabled) => {
                if self.mic_gate != enabled {
                    self.mic_gate = enabled;
                    if self.mic_wanted() {
                        if self.mic_modules.is_empty() {
                            self.request_mic_chain();
                        } else {
                            // Stage membership changed — rewire.
                            self.reload_mic_chain();
                        }
                    } else {
                        self.disable_mic_fx(true);
                    }
                }
            }
            BackendCommand::SetMicEffectValue { effect, value } => {
                self.set_mic_effect_value(effect, value)
            }
            BackendCommand::SetMicMonitor(on) => self.set_mic_monitor(on),
            BackendCommand::SetMeterTarget(id) => self.set_meter_target(id),
            BackendCommand::Quit => {}
        }
    }

    /// Push the full endpoint snapshot to the GUI.
    pub(super) fn emit_endpoints(&self) {
        let mut list: Vec<AudioEndpoint> = self.endpoints.values().cloned().collect();
        list.sort_by(|a, b| a.description.cmp(&b.description));
        let _ = self.event_tx.send(BackendEvent::EndpointsChanged(list));
    }
}

/// Helper for `DictRef` lookups with borrowed `&str` results.
fn dict_get<'a>(dict: &'a DictRef, key: &str) -> Option<&'a str> {
    dict.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}
