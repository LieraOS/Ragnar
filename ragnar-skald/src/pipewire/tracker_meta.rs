// Copyright (C) Jörg Schröder
//! Session-default handling for the [`Tracker`].
//!
//! PipeWire clients pick their default sink/source from WirePlumber's
//! "default" metadata object — the same store `wpctl set-default`
//! writes. This module keeps the binding of that object, the
//! `default.configured.audio.*` property parsing and the recomputation
//! of each endpoint's `is_default` flag in one place.

use pipewire::metadata::{Metadata, MetadataListener};
use tracing::{debug, warn};

use super::tracker::Tracker;
use super::types::Direction;

impl Tracker {
    /// Store the bound "default" metadata object and flush any
    /// deferred fx-enable request — the default-sink reroute in
    /// `enable_fx` needs this object to exist.
    pub(super) fn bind_default_metadata(&mut self, metadata: Metadata, listener: MetadataListener) {
        self.default_metadata = Some((metadata, listener));
        debug!("Bound default metadata object");

        if self.fx_pending {
            self.fx_pending = false;
            self.enable_fx();
        }
        // The mic chain additionally needs an enumerated real source —
        // if none has shown up yet, `add_node` retries on arrival.
        if self.mic_pending && self.real_source_name().is_some() {
            self.mic_pending = false;
            self.enable_mic_fx();
        }
    }

    /// Property event on the "default" metadata object.
    pub(super) fn apply_default_property(&mut self, key: Option<&str>, value: Option<&str>) {
        let Some(key) = key else { return };
        let direction = if key == Direction::Output.default_metadata_key() {
            Direction::Output
        } else if key == Direction::Input.default_metadata_key() {
            Direction::Input
        } else {
            return;
        };

        // The value is a JSON object like {"name":"alsa_output..."}.
        let name = value.and_then(|v| {
            serde_json::from_str::<serde_json::Value>(v)
                .ok()
                .and_then(|j| j.get("name")?.as_str().map(String::from))
        });
        let is_real_input = direction == Direction::Input
            && name.as_deref().is_some_and(|n| !n.starts_with("skald."));
        match name {
            Some(n) => self.default_names.insert(direction, n),
            None => self.default_names.remove(&direction),
        };

        // Input-side re-anchor: the virtual mic is the permanent
        // session default. Selecting a real source (GUI, pavucontrol,
        // wpctl) means "use THAT mic" — rebuild the chain so its
        // capture follows, then hand the default back to mic.out.
        if is_real_input && !self.mic_modules.is_empty() {
            self.disable_mic_fx(false);
            self.enable_mic_fx();
        } else {
            self.refresh_defaults();
            self.emit_endpoints();
        }
    }

    /// Recompute `is_default` on every endpoint after defaults changed.
    pub(super) fn refresh_defaults(&mut self) {
        for ep in self.endpoints.values_mut() {
            ep.is_default = self
                .default_names
                .get(&ep.direction)
                .map(|n| n == &ep.name)
                .unwrap_or(false);
        }
    }

    /// Current configured default name for `direction`, if any.
    pub(super) fn default_name(&self, direction: Direction) -> Option<&str> {
        self.default_names.get(&direction).map(String::as_str)
    }

    /// Make `name` the configured default endpoint for `direction` by
    /// writing the "default" metadata object (WirePlumber's store).
    pub(super) fn set_default(&self, name: &str, direction: Direction) {
        let Some((metadata, _listener)) = &self.default_metadata else {
            warn!("SetDefault: default metadata object not available");
            return;
        };
        let value = serde_json::json!({ "name": name }).to_string();
        metadata.set_property(
            0,
            direction.default_metadata_key(),
            Some("Spa:String:JSON"),
            Some(&value),
        );
        debug!("Set default {:?} endpoint to {name}", direction);
    }
}
