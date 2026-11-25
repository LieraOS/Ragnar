// Copyright (C) Jörg Schröder
//! Effects widget — built-in filter-chain effects.
//!
//! The output chain is owned by the `ragnar-skald-fx` daemon so it
//! survives the window closing (the user expects effects to stay
//! system-wide until disabled). The enable checkbox writes the intent
//! into settings and spawns the daemon; sliders push live `params`
//! onto the daemon-owned nodes — PipeWire accepts control writes from
//! any client — and persist for the daemon's next rebuild.
//!
//! `FxStateChanged` reflects the *presence* of `skald.fx.*` nodes
//! in the registry, so the checkbox shows the real chain state
//! regardless of who owns the modules.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{Box, CheckButton, Label, Orientation, Scale, Widget};
use tracing::{debug, warn};

use crate::audio::AudioEffect;
use crate::config::{AppSettings, ConfigPersistence};
use crate::gui::daemon_spawn::spawn_daemon;
use crate::gui::debounced_save::{debounced_save, pending_slot, save_now};
use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{BackendEvent, FxEffect};

/// One effect slider row: label + scale bound to a [`FxEffect`].
struct EffectSlider {
    effect: FxEffect,
    scale: Scale,
}

/// Built-in effects section shown below the output device controls.
pub struct EffectsWidget {
    container: Box,
    enable_check: CheckButton,
    sliders: Vec<EffectSlider>,
    sliders_box: Box,
    status: Label,
    backend: Rc<PwBackend>,
    settings: Rc<RefCell<AppSettings>>,
    persist: Rc<ConfigPersistence>,
    /// True while widgets mirror backend/persisted state — suppresses
    /// command echo back into the backend.
    updating: Rc<Cell<bool>>,
    last_spawn: Rc<Cell<Option<Instant>>>,
}

impl EffectsWidget {
    /// Create the effects section.
    ///
    /// `settings`/`persist` are shared with the application so effect
    /// choices survive restarts; `restore()` applies the saved state to
    /// both widgets and backend.
    pub fn new(
        backend: Rc<PwBackend>,
        settings: Rc<RefCell<AppSettings>>,
        persist: Rc<ConfigPersistence>,
    ) -> Self {
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(6)
            .build();

        let title = Label::builder()
            .label("Effekte")
            .halign(gtk4::Align::Start)
            .css_classes(["title-4"])
            .build();
        container.append(&title);

        let enable_check = CheckButton::builder()
            .label("Effekte aktivieren")
            .halign(gtk4::Align::Start)
            .build();
        container.append(&enable_check);

        let status = Label::builder()
            .halign(gtk4::Align::Start)
            .css_classes(["dim-label"])
            .build();
        container.append(&status);

        let sliders_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .sensitive(false) // locked until the chain is loaded
            .build();
        container.append(&sliders_box);

        let sliders = vec![
            Self::add_slider(&sliders_box, FxEffect::Bass, "Bass (dB)", -15.0, 15.0),
            Self::add_slider(&sliders_box, FxEffect::Treble, "Höhen (dB)", -15.0, 15.0),
            Self::add_slider(&sliders_box, FxEffect::Echo, "Echo", 0.0, 10.0),
            Self::add_slider(&sliders_box, FxEffect::Reverb, "Hall", 0.0, 10.0),
        ];

        let widget = Self {
            container,
            enable_check,
            sliders,
            sliders_box,
            status,
            backend,
            settings,
            persist,
            updating: Rc::new(Cell::new(false)),
            last_spawn: Rc::new(Cell::new(None)),
        };
        widget.connect_signals();
        widget
    }

    /// Build one labelled slider row for `effect`.
    fn add_slider(parent: &Box, effect: FxEffect, label: &str, min: f64, max: f64) -> EffectSlider {
        let row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(
            &Label::builder()
                .label(label)
                .halign(gtk4::Align::Start)
                .width_chars(10)
                .build(),
        );
        // Bipolar sliders (EQ gain in dB) get the mixer look: the
        // fill runs from the center out to the handle instead of from
        // the left edge, plus a tick at 0 so "no effect" is visible.
        let bipolar = min < 0.0;
        let scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&gtk4::Adjustment::new(0.0, min, max, 0.5, 1.0, 0.0))
            .hexpand(true)
            .value_pos(gtk4::PositionType::Right)
            .has_origin(!bipolar)
            .draw_value(true)
            .digits(0)
            .build();
        if bipolar {
            scale.add_mark(0.0, gtk4::PositionType::Bottom, None);
        }
        row.append(&scale);
        parent.append(&row);
        EffectSlider { effect, scale }
    }

    /// Get the top-level widget for embedding.
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Apply persisted settings: widget values plus backend commands.
    /// Call after the backend is connected — enables the chain again if
    /// it was on last session.
    pub fn restore(&self) {
        let cfg = self.settings.borrow().audio_config.clone();
        self.updating.set(true);
        self.enable_check.set_active(cfg.fx_enabled);
        for s in &self.sliders {
            s.scale.set_value(self.saved_value(s.effect, &cfg));
        }
        self.updating.set(false);
        self.sliders_box.set_sensitive(cfg.fx_enabled);

        if cfg.fx_enabled {
            spawn_daemon(&self.last_spawn, &self.status);
            self.push_all_values();
        }
    }

    /// Turn the effects off exactly as if the user had unticked the
    /// checkbox — goes through the proven path (settings write,
    /// persist, daemon unload). Called when the user picks a raw
    /// output device while the chain is still loaded: selecting real
    /// hardware means "no effects", so the chain must come down
    /// instead of feeding the sink in parallel to the direct route.
    pub fn disable(&self) {
        if self.enable_check.is_active() {
            self.enable_check.set_active(false);
        }
    }

    /// Saved slider value for `effect` read from `cfg`.
    fn saved_value(&self, effect: FxEffect, cfg: &crate::audio::AudioConfig) -> f64 {
        match effect {
            FxEffect::Bass => f64::from(cfg.equalizer.bass()),
            FxEffect::Treble => f64::from(cfg.equalizer.treble()),
            FxEffect::Echo => f64::from(cfg.echo.intensity()),
            FxEffect::Reverb => f64::from(cfg.reverb.intensity()),
        }
    }

    /// Send every slider's value to the backend — queued values are
    /// replayed when the daemon's capture nodes bind in our registry.
    fn push_all_values(&self) {
        for s in &self.sliders {
            self.backend
                .set_effect_value(s.effect, s.scale.value() as f32);
        }
    }

    /// Route a backend event (fx state changes) into the widget.
    pub fn apply_event(&self, event: &BackendEvent) {
        if let BackendEvent::FxStateChanged(enabled) = event {
            debug!("Fx chain state: {enabled}");
            self.updating.set(true);
            self.enable_check.set_active(*enabled);
            self.updating.set(false);
            self.sliders_box.set_sensitive(*enabled);
            // Settings say enabled but the chain vanished — the daemon
            // probably died; respawn it (cooldown-guarded).
            if !*enabled && self.settings.borrow().audio_config.fx_enabled {
                spawn_daemon(&self.last_spawn, &self.status);
            }
        }
    }

    /// Wire widget signals: enable toggle and per-slider commands.
    fn connect_signals(&self) {
        let pending = pending_slot();

        // Master toggle: persist intent, then let the daemon apply it.
        // The daemon watches settings.json — it loads/unloads the chain
        // and exits by itself once everything is disabled.
        let settings = Rc::clone(&self.settings);
        let persist = Rc::clone(&self.persist);
        let updating = Rc::clone(&self.updating);
        let last_spawn = Rc::clone(&self.last_spawn);
        let status = self.status.clone();
        let box_ = self.sliders_box.clone();
        self.enable_check.connect_toggled(move |check| {
            if updating.get() {
                return;
            }
            let enabled = check.is_active();
            settings.borrow_mut().audio_config.fx_enabled = enabled;
            save_now(&settings, &persist);
            // Controls unlock as soon as FxStateChanged confirms;
            // pre-unlock for immediate visual feedback.
            box_.set_sensitive(enabled);
            if enabled {
                spawn_daemon(&last_spawn, &status);
            }
        });

        for s in &self.sliders {
            let backend = Rc::clone(&self.backend);
            let settings = Rc::clone(&self.settings);
            let updating = Rc::clone(&self.updating);
            let effect = s.effect;
            let save = debounced_save(
                Rc::clone(&self.settings),
                Rc::clone(&self.persist),
                Rc::clone(&pending),
            );
            s.scale.connect_value_changed(move |scale| {
                if updating.get() {
                    return;
                }
                let v = scale.value();
                backend.set_effect_value(effect, v as f32);
                // Persist into the matching model field.
                let mut st = settings.borrow_mut();
                let cfg = &mut st.audio_config;
                match effect {
                    FxEffect::Bass => {
                        if let Err(e) = cfg.equalizer.set_bass(v as i8) {
                            warn!("Bass value rejected: {e}");
                        }
                    }
                    FxEffect::Treble => {
                        if let Err(e) = cfg.equalizer.set_treble(v as i8) {
                            warn!("Treble value rejected: {e}");
                        }
                    }
                    FxEffect::Echo => {
                        let _ = cfg.echo.set_intensity(v as u8);
                        cfg.echo.set_enabled(v > 0.0);
                    }
                    FxEffect::Reverb => {
                        let _ = cfg.reverb.set_intensity(v as u8);
                        cfg.reverb.set_enabled(v > 0.0);
                    }
                }
                drop(st);
                save();
            });
        }
    }
}
