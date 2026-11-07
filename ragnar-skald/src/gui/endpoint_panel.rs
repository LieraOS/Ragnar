//! Endpoint control panel for one audio direction.
//!
//! Reusable composite used by both the output and the input tab: a
//! device list (GNOME sound-settings style — the system `DropDown`
//! widget is unusable on some GTK builds, see history notes), a volume
//! slider with optional 150 % boost, and a mute switch. All controls
//! talk to the PipeWire worker through [`PwBackend`] commands; incoming
//! `BackendEvent`s update the widgets without re-emitting commands
//! (feedback-guarded via `updating`).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, CheckButton, Label, ListBox, ListBoxRow, Orientation, Scale, Widget};

use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{AudioEndpoint, BackendEvent, Direction, MAX_VOLUME};

/// Shared slot for the optional endpoint-selection callback
/// (`set_on_select`) — boxed to keep the struct readable.
type OnSelect = Rc<RefCell<Option<std::boxed::Box<dyn Fn(u32)>>>>;

/// Callback for "raw device picked while the fx chain is loaded".
type OnRawSelect = Rc<RefCell<Option<std::boxed::Box<dyn Fn()>>>>;

/// Row label: description plus a marker suffix for the session default.
fn endpoint_label(ep: &AudioEndpoint) -> String {
    if ep.is_default {
        format!("{}  ✓", ep.description)
    } else {
        ep.description.clone()
    }
}

/// Panel controlling all endpoints of one direction.
pub struct EndpointPanel {
    container: Box,
    direction: Direction,
    backend: Rc<PwBackend>,
    /// Known endpoints keyed by node id (BTreeMap = stable order).
    endpoints: Rc<RefCell<BTreeMap<u32, AudioEndpoint>>>,
    /// Registry ids in the same order as the list rows.
    row_ids: Rc<RefCell<Vec<u32>>>,
    /// Node currently controlled by the sliders.
    selected_id: Rc<Cell<Option<u32>>>,
    /// True while widgets are updated from backend events — prevents
    /// signal handlers from echoing commands back to the backend.
    updating: Rc<Cell<bool>>,
    device_list: ListBox,
    scale: Scale,
    percent_label: Label,
    mute_check: CheckButton,
    boost_check: CheckButton,
    status: Label,
    /// Notified whenever the selected endpoint changes — the input
    /// panel uses it to aim the level meter at the active mic.
    on_select: OnSelect,
    /// Fired when the user picks a raw device while the fx chain is
    /// loaded — the owner turns the chain off through the proven path.
    on_raw_select: OnRawSelect,
}

impl EndpointPanel {
    /// Create a panel for `direction`.
    pub fn new(direction: Direction, backend: Rc<PwBackend>) -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        let title = Label::builder()
            .label(match direction {
                Direction::Output => "Ausgabegeräte",
                Direction::Input => "Eingabegeräte",
            })
            .halign(gtk4::Align::Start)
            .css_classes(["title-4"])
            .build();
        container.append(&title);

        let status = Label::builder()
            .label("Verbinde mit PipeWire…")
            .halign(gtk4::Align::Start)
            .build();
        container.append(&status);

        // Device list — activating a row selects that endpoint and makes
        // it the session default for this direction.
        container.append(
            &Label::builder()
                .label("Gerät")
                .halign(gtk4::Align::Start)
                .build(),
        );
        let device_list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .hexpand(true)
            .sensitive(false)
            .css_classes(["boxed-list"])
            .build();
        container.append(&device_list);

        // Volume slider — 0-100 %, extended to 150 % while boost is on.
        let volume_row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        volume_row.append(&Label::builder().label("Lautstärke").build());
        let scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&gtk4::Adjustment::new(100.0, 0.0, 100.0, 1.0, 10.0, 0.0))
            .hexpand(true)
            .sensitive(false)
            .build();
        volume_row.append(&scale);
        let percent_label = Label::builder().label("—").width_chars(6).build();
        volume_row.append(&percent_label);
        container.append(&volume_row);

        // Mute + boost row.
        let toggles = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .build();
        let mute_check = CheckButton::builder()
            .label("Stumm")
            .sensitive(false)
            .build();
        toggles.append(&mute_check);
        // Boost is an output-only feature: on input endpoints the
        // slider drives the hardware ADC gain directly, and any value
        // beyond unity clips the capture path hard — input must stay
        // at or below 100 %.
        let boost_check = CheckButton::builder()
            .label("Verstärkung (bis 150 %)")
            .sensitive(false)
            .visible(direction == Direction::Output)
            .build();
        toggles.append(&boost_check);
        container.append(&toggles);

        let panel = Self {
            container,
            direction,
            backend,
            endpoints: Rc::new(RefCell::new(BTreeMap::new())),
            row_ids: Rc::new(RefCell::new(Vec::new())),
            selected_id: Rc::new(Cell::new(None)),
            updating: Rc::new(Cell::new(false)),
            device_list,
            scale,
            percent_label,
            mute_check,
            boost_check,
            status,
            on_select: Rc::new(RefCell::new(None)),
            on_raw_select: Rc::new(RefCell::new(None)),
        };
        panel.connect_signals();
        panel
    }

    /// Register a callback fired whenever the selected endpoint changes
    /// (user click or default-driven reselection after a rebuild).
    pub fn set_on_select(&self, cb: impl Fn(u32) + 'static) {
        *self.on_select.borrow_mut() = Some(std::boxed::Box::new(cb));
    }

    /// Register a callback fired when the user picks a raw device
    /// while the fx chain is still loaded — the owner is expected to
    /// run the proven "effects off" path.
    pub fn set_on_raw_select(&self, cb: impl Fn() + 'static) {
        *self.on_raw_select.borrow_mut() = Some(std::boxed::Box::new(cb));
    }

    /// Get the top-level widget for embedding.
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Apply a backend event to this panel.
    pub fn apply_event(&self, event: &BackendEvent) {
        match event {
            BackendEvent::EndpointsChanged(list) => {
                *self.endpoints.borrow_mut() = list
                    .iter()
                    .filter(|ep| ep.direction == self.direction)
                    .map(|ep| (ep.id, ep.clone()))
                    .collect();
                self.rebuild_list();
            }
            BackendEvent::EndpointChanged(ep) => {
                if ep.direction == self.direction {
                    self.endpoints.borrow_mut().insert(ep.id, ep.clone());
                    if self.selected_id.get() == Some(ep.id) {
                        self.sync_controls(ep);
                    }
                    // A default flag may have flipped — refresh labels.
                    self.refresh_row_labels();
                }
            }
            BackendEvent::FxStateChanged(_)
            | BackendEvent::MicFxStateChanged(_)
            | BackendEvent::MicLevel(_)
            | BackendEvent::StreamsChanged(_)
            | BackendEvent::StreamChanged(_) => {
                // Effects widgets / app-stream cards handle those.
            }
            BackendEvent::BackendLost(msg) => {
                self.status
                    .set_label(&format!("PipeWire-Verbindung verloren: {msg}"));
                self.set_controls_sensitive(false);
            }
        }
    }

    /// Rebuild the device list rows; keeps the previous selection when
    /// the node still exists, otherwise selects the session default.
    fn rebuild_list(&self) {
        self.updating.set(true);
        let endpoints = self.endpoints.borrow();
        let list: Vec<&AudioEndpoint> = endpoints.values().collect();

        // Clear previous rows.
        while let Some(child) = self.device_list.first_child() {
            self.device_list.remove(&child);
        }
        *self.row_ids.borrow_mut() = list.iter().map(|ep| ep.id).collect();

        if list.is_empty() {
            self.status.set_label("Keine Geräte gefunden");
            self.selected_id.set(None);
            self.set_controls_sensitive(false);
            self.device_list.set_sensitive(false);
            self.updating.set(false);
            return;
        }

        self.status.set_label("");
        self.device_list.set_sensitive(true);

        for ep in &list {
            let row = ListBoxRow::new();
            let label = Label::builder()
                .label(endpoint_label(ep))
                .halign(gtk4::Align::Start)
                .margin_start(8)
                .margin_end(8)
                .margin_top(4)
                .margin_bottom(4)
                .build();
            row.set_child(Some(&label));
            self.device_list.append(&row);
        }

        // Prefer the previously selected node, fall back to the default.
        let position = list
            .iter()
            .position(|ep| self.selected_id.get() == Some(ep.id))
            .or_else(|| list.iter().position(|ep| ep.is_default))
            .unwrap_or(0);
        if let Some(row) = self.device_list.row_at_index(position as i32) {
            self.device_list.select_row(Some(&row));
        }
        // Notify the selection callback only on real changes — list
        // rebuilds happen on every node add/remove and would otherwise
        // churn the meter stream.
        let new_id = list[position].id;
        if self.selected_id.get() != Some(new_id) {
            self.selected_id.set(Some(new_id));
            if let Some(cb) = self.on_select.borrow().as_ref() {
                cb(new_id);
            }
        }
        self.sync_controls(list[position]);
        self.updating.set(false);
    }

    /// Update row labels (default marker) without rebuilding the list.
    fn refresh_row_labels(&self) {
        self.updating.set(true);
        let endpoints = self.endpoints.borrow();
        let ids = self.row_ids.borrow();
        let mut index = 0;
        while let Some(row) = self.device_list.row_at_index(index) {
            if let Some(ep) = ids.get(index as usize).and_then(|id| endpoints.get(id)) {
                if let Some(label) = row.child().and_then(|w| w.downcast::<Label>().ok()) {
                    label.set_label(&endpoint_label(ep));
                }
            }
            index += 1;
        }
        self.updating.set(false);
    }

    /// Mirror an endpoint's state into the widgets (guarded).
    fn sync_controls(&self, ep: &AudioEndpoint) {
        self.updating.set(true);
        let pct = f64::from(ep.volume) * 100.0;
        // Boost is a user-held preference: auto-ENABLE it when the real
        // volume exceeds 100 % (reflecting actual state), but never
        // auto-disable — dropping below 100 % must not switch it off.
        if pct > 100.0 {
            self.boost_check.set_active(true);
        }
        self.scale
            .set_value(pct.clamp(0.0, self.scale.adjustment().upper()));
        self.percent_label.set_label(&format!("{pct:.0} %"));
        self.mute_check.set_active(ep.muted);
        // Controls stay locked until the first Props param arrived.
        self.set_controls_sensitive(ep.volume_known());
        self.updating.set(false);
    }

    fn set_controls_sensitive(&self, sensitive: bool) {
        self.scale.set_sensitive(sensitive);
        self.mute_check.set_sensitive(sensitive);
        self.boost_check.set_sensitive(sensitive);
    }

    /// Wire all widget signals to backend commands.
    fn connect_signals(&self) {
        // Row selection = choose endpoint + make it the session default.
        let endpoints = Rc::clone(&self.endpoints);
        let ids = Rc::clone(&self.row_ids);
        let selected_id = Rc::clone(&self.selected_id);
        let backend = Rc::clone(&self.backend);
        let updating = Rc::clone(&self.updating);
        let direction = self.direction;
        let scale = self.scale.clone();
        let percent = self.percent_label.clone();
        let mute = self.mute_check.clone();
        let boost = self.boost_check.clone();
        let on_select = Rc::clone(&self.on_select);
        let on_raw_select = Rc::clone(&self.on_raw_select);
        self.device_list.connect_row_selected(move |_list, row| {
            if updating.get() {
                return;
            }
            let Some(row) = row else { return };
            let idx = row.index();
            if idx < 0 {
                return;
            }
            let Some(&id) = ids.borrow().get(idx as usize) else {
                return;
            };
            selected_id.set(Some(id));
            if let Some(cb) = on_select.borrow().as_ref() {
                cb(id);
            }
            let map = endpoints.borrow();
            if let Some(ep) = map.get(&id) {
                // Split-state guard: while a chain is loaded, a real
                // device must never be selected directly. Output: apps
                // would route around the fx chain while its output
                // link still feeds the sink — summed signal clips.
                // Input: apps bypass the mic chain while its stages
                // keep running — the filter then "does nothing" for
                // no visible reason. Picking raw hardware = the chain
                // must go down first.
                if !ep.name.starts_with("skald.")
                    && map.values().any(|e| e.name.starts_with("skald."))
                {
                    if let Some(cb) = on_raw_select.borrow().as_ref() {
                        cb();
                    }
                }
                backend.set_default(&ep.name, direction);
                // Mirror state into controls without echoing commands.
                updating.set(true);
                let pct = f64::from(ep.volume) * 100.0;
                // Same rule as sync_controls: never auto-disable boost.
                if pct > 100.0 {
                    boost.set_active(true);
                }
                scale.set_value(pct.clamp(0.0, scale.adjustment().upper()));
                percent.set_label(&format!("{pct:.0} %"));
                mute.set_active(ep.muted);
                updating.set(false);
            }
        });

        // Slider → flat volume on all channels.
        let backend = Rc::clone(&self.backend);
        let selected_id = Rc::clone(&self.selected_id);
        let updating = Rc::clone(&self.updating);
        let percent = self.percent_label.clone();
        self.scale.connect_value_changed(move |scale| {
            let pct = scale.value();
            percent.set_label(&format!("{pct:.0} %"));
            if updating.get() {
                return;
            }
            if let Some(id) = selected_id.get() {
                backend.set_volume(id, (pct / 100.0) as f32);
            }
        });

        // Mute checkbox → Props.mute.
        // (gtk4::Switch is unusable on some GTK 4.18 builds — it
        // stack-overflows in layout — so a CheckButton carries mute.)
        let backend = Rc::clone(&self.backend);
        let selected_id = Rc::clone(&self.selected_id);
        let updating = Rc::clone(&self.updating);
        self.mute_check.connect_toggled(move |check| {
            if !updating.get() {
                if let Some(id) = selected_id.get() {
                    backend.set_mute(id, check.is_active());
                }
            }
        });

        // Boost checkbox widens the slider range; disabling clamps to 100 %.
        let scale = self.scale.clone();
        let updating = Rc::clone(&self.updating);
        self.boost_check.connect_toggled(move |check| {
            let upper = if check.is_active() {
                f64::from(MAX_VOLUME) * 100.0
            } else {
                100.0
            };
            scale.adjustment().set_upper(upper);
            if !check.is_active() && scale.value() > 100.0 && !updating.get() {
                scale.set_value(100.0); // emits value_changed → backend
            }
        });
    }
}
