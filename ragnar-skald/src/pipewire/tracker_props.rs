//! Endpoint `Props` handling: applies param events to the endpoint
//! model and serializes volume/mute writes onto node proxies.
//!
//! Split out of `tracker.rs` to keep each module under the project
//! size limit — same `impl Tracker` block, same file-internal rules.

use libspa::param::ParamType;
use libspa::pod::Pod;
use pipewire::node::Node;
use tracing::warn;

use super::fxchain;
use super::props::{self, PropsState};
use super::tracker::Tracker;
use super::types::{BackendEvent, MAX_VOLUME};

impl Tracker {
    /// A `Props` param arrived for a node: fold volume/mute into state
    /// and notify the GUI if anything actually changed.
    pub(super) fn apply_props(&mut self, id: u32, state: PropsState) {
        if let Some(stream) = self.streams.get_mut(&id) {
            let mut changed = false;
            if let Some(volumes) = state.channel_volumes {
                // channelVolumes arrive LINEAR — the model holds the
                // cubic display convention (wpctl/pavucontrol/taskbar),
                // so the average gets converted with cbrt.
                let avg = volumes.iter().sum::<f32>() / volumes.len().max(1) as f32;
                let v = avg.cbrt();
                if (stream.volume - v).abs() > f32::EPSILON {
                    stream.volume = v;
                    changed = true;
                }
            } else if let Some(volume) = state.volume {
                if (stream.volume - volume).abs() > f32::EPSILON {
                    stream.volume = volume;
                    changed = true;
                }
            }
            if let Some(mute) = state.mute {
                if stream.muted != mute {
                    stream.muted = mute;
                    changed = true;
                }
            }
            if changed {
                let _ = self
                    .event_tx
                    .send(BackendEvent::StreamChanged(stream.clone()));
            }
            return;
        }
        // Internal fx `.out` plumbing must always sit at unity — any
        // restored or foreign volume multiplies through the chain.
        if self.fx_plumbing.contains(&id) {
            let over = state
                .channel_volumes
                .map(|v| (v.iter().sum::<f32>() / v.len().max(1) as f32 - 1.0).abs() > f32::EPSILON)
                .unwrap_or(false)
                || state.volume.is_some_and(|v| (v - 1.0).abs() > f32::EPSILON);
            if over {
                self.set_volume(id, 1.0);
            }
            return;
        }
        // First volume landing on an fx `.in` endpoint gets overridden
        // with the carried-over sink level — the WP-restored value can
        // be a boosted leftover that multiplies every routed stream.
        if let Some(v) = self.pending_fx_volume {
            if self
                .endpoints
                .get(&id)
                .is_some_and(|e| fxchain::is_fx_node(&e.name) && e.name.ends_with(".in"))
            {
                self.pending_fx_volume = None;
                self.set_volume(id, v);
                return;
            }
        }
        let Some(ep) = self.endpoints.get_mut(&id) else {
            return;
        };
        let mut changed = false;

        if let Some(volumes) = state.channel_volumes {
            if ep.channel_volumes != volumes {
                // channelVolumes are LINEAR; `volume` is the cubic
                // master (same convention as pavucontrol/wpctl/the
                // taskbar) — display the cbrt of the channel average.
                ep.volume = (volumes.iter().sum::<f32>() / volumes.len().max(1) as f32).cbrt();
                ep.channel_volumes = volumes;
                changed = true;
            }
        } else if let Some(volume) = state.volume {
            // Scalar `volume` is only a fallback for nodes that expose no
            // channelVolumes — otherwise it would clobber the per-channel
            // average (PipeWire reports 1.0 here for flat sinks).
            if (ep.volume - volume).abs() > f32::EPSILON {
                ep.volume = volume;
                changed = true;
            }
        }
        if let Some(mute) = state.mute {
            if ep.muted != mute {
                ep.muted = mute;
                changed = true;
            }
        }

        if changed {
            let _ = self
                .event_tx
                .send(BackendEvent::EndpointChanged(ep.clone()));
        }
    }

    /// Write a `Props` param on the node with flat volume for all channels.
    pub(super) fn set_volume(&self, id: u32, volume: f32) {
        let Some(handle) = self.nodes.get(&id) else {
            warn!("SetVolume for unknown node {id}");
            return;
        };
        let volume = volume.clamp(0.0, MAX_VOLUME);
        // Callers pass the CUBIC display value (GUI sliders, carry-over
        // restores) — channelVolumes are LINEAR gains, so each channel
        // gets volume³; the scalar prop keeps the cubic value via
        // build_volume_pod's own derivation.
        let linear = volume * volume * volume;
        let channels = self
            .endpoints
            .get(&id)
            .map(|ep| ep.channel_volumes.len().max(1))
            .unwrap_or(2);
        let channel_volumes = vec![linear; channels];

        match props::build_volume_pod(&channel_volumes, None) {
            Ok(bytes) => {
                if let Some(pod) = Pod::from_bytes(&bytes) {
                    handle.proxy.set_param(ParamType::Props, 0, pod);
                }
            }
            Err(e) => warn!("Failed to serialize volume pod: {e}"),
        }
    }

    /// Write a `Props` param carrying only the mute flag.
    pub(super) fn set_mute(&self, id: u32, muted: bool) {
        let Some(handle) = self.nodes.get(&id) else {
            warn!("SetMute for unknown node {id}");
            return;
        };
        match props::build_mute_pod(muted) {
            Ok(bytes) => {
                if let Some(pod) = Pod::from_bytes(&bytes) {
                    handle.proxy.set_param(ParamType::Props, 0, pod);
                }
            }
            Err(e) => warn!("Failed to serialize mute pod: {e}"),
        }
    }

    /// Node proxy accessor for the sibling fx-control modules.
    pub(super) fn node_proxy(&self, id: u32) -> Option<&Node> {
        self.nodes.get(&id).map(|h| &h.proxy)
    }
}
