//! Microphone effects widget — input-chain controls.
//!
//! Controls the mic effects chain owned by the `ragnar-skald-fx` daemon.
//! Two independent concerns live here:
//!
//! - **Effects master switch** (voice presets + echo) — persists
//!   `mic_fx_enabled` and spawns the daemon. The checkbox is pure
//!   user intent: chain presence events NEVER rewrite it (a rebuild
//!   makes `skald.mic.out` vanish for a moment — mirroring that
//!   into the checkbox used to switch the whole section off).
//! - **Noise gate** — always usable, works with or without the fun
//!   effects; the backend runs the chain for the gate alone.
//!
//! Chain presence (`MicFxStateChanged`) only drives control
//! sensitivity, the status label and the daemon-respawn watchdog.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{Box, CheckButton, Label, LevelBar, Orientation, Scale, ToggleButton, Widget};
use tracing::debug;

use crate::config::{AppSettings, ConfigPersistence};
use crate::gui::daemon_spawn::spawn_daemon;
use crate::gui::debounced_save::{debounced_save, pending_slot, save_now};
use crate::pipewire::backend::PwBackend;
use crate::pipewire::micfx;
use crate::pipewire::types::{BackendEvent, MicEffect, VoicePreset};

/// Section heading (`title-4` style), left aligned.
fn title_label(text: &str) -> Label {
    Label::builder()
        .label(text)
        .halign(gtk4::Align::Start)
        .css_classes(["title-4"])
        .build()
}

/// Fixed-width caption used by the level row and slider rows.
fn row_label(text: &str) -> Label {
    Label::builder()
        .label(text)
        .halign(gtk4::Align::Start)
        .width_chars(14)
        .build()
}

/// Microphone effects section of the input tab.
pub struct MicEffectsWidget {
    container: Box,
    enable_check: CheckButton,
    level_bar: LevelBar,
    preset_checks: Vec<(VoicePreset, CheckButton)>,
    gate_check: CheckButton,
    gate_scale: Scale,
    pitch_scale: Scale,
    echo_scale: Scale,
    fx_box: Box,
    status: Label,
    settings: Rc<RefCell<AppSettings>>,
    /// True while widgets mirror persisted state (restore path).
    updating: Rc<Cell<bool>>,
    last_spawn: Rc<Cell<Option<Instant>>>,
}

impl MicEffectsWidget {
    /// Create the mic effects section.
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

        container.append(&title_label("Mikrofon"));

        // Live input level — visible proof the mic is really alive.
        let level_row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        level_row.append(&row_label("Pegel"));
        let level_bar = LevelBar::builder()
            .hexpand(true)
            .min_value(0.0)
            .max_value(1.0)
            .mode(gtk4::LevelBarMode::Continuous)
            .build();
        level_row.append(&level_bar);
        container.append(&level_row);

        // Noise gate — independent of the fun effects, always usable.
        let gate_available = micfx::denoise_available();
        let gate_label = if gate_available {
            "Rauschunterdrückung".to_string()
        } else {
            "Rauschunterdrückung (Plugin fehlt)".to_string()
        };
        let gate_check = CheckButton::builder()
            .label(gate_label)
            .sensitive(gate_available)
            .build();
        container.append(&gate_check);
        let gate_scale = Self::add_slider(&container, "Schwelle", 0.0, 10.0);
        gate_scale.set_sensitive(gate_available);

        container.append(&gtk4::Separator::new(Orientation::Horizontal));
        container.append(&title_label("Stimmenfilter"));

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

        let fx_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .sensitive(false)
            .build();
        container.append(&fx_box);

        // Voice presets — grouped radio-style checkboxes. The master
        // switch above IS the "off" state, so there is no "Aus" entry.
        let preset_labels = [
            (VoicePreset::Mickey, "Micky Maus"),
            (VoicePreset::Robot, "Roboter"),
            (VoicePreset::Female, "Frauenstimme"),
            (VoicePreset::Male, "Männerstimme"),
        ];
        let mut preset_checks = Vec::new();
        let mut group_anchor: Option<CheckButton> = None;
        for (preset, text) in preset_labels {
            let available = micfx::preset_available(preset);
            let label = if available {
                text.to_string()
            } else {
                format!("{text} (Plugin fehlt)")
            };
            let check = CheckButton::builder()
                .label(label)
                .sensitive(available)
                .build();
            if let Some(first) = &group_anchor {
                check.set_group(Some(first));
            } else {
                group_anchor = Some(check.clone());
            }
            fx_box.append(&check);
            preset_checks.push((preset, check));
        }

        // Live pitch retune — absolute semitones around the preset
        // default; only pitch-shifted presets have a shifter to drive.
        let pitch_scale = Self::add_slider(&fx_box, "Tonhöhe", -12.0, 12.0);
        pitch_scale.set_sensitive(false);

        // Light mic echo.
        let echo_scale = Self::add_slider(&fx_box, "Echo", 0.0, 10.0);

        // "Hear yourself" test button — the loopback module lives in
        // the GUI process and dies with the window, so a forgotten
        // active test can never become a stale feedback path.
        let monitor_button = ToggleButton::builder()
            .label("Testen — Anhören (Kopfhörer empfohlen)")
            .halign(gtk4::Align::Start)
            .margin_top(6)
            .build();
        container.append(&monitor_button);

        let widget = Self {
            container,
            enable_check,
            level_bar,
            preset_checks,
            gate_check,
            gate_scale,
            pitch_scale,
            echo_scale,
            fx_box,
            status,
            settings,
            updating: Rc::new(Cell::new(false)),
            last_spawn: Rc::new(Cell::new(None)),
        };
        widget.connect_signals(backend, persist, monitor_button);
        widget
    }

    /// Build one labelled slider row.
    fn add_slider(parent: &Box, label: &str, min: f64, max: f64) -> Scale {
        let row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        row.append(&row_label(label));
        let scale = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&gtk4::Adjustment::new(0.0, min, max, 0.5, 1.0, 0.0))
            .hexpand(true)
            .value_pos(gtk4::PositionType::Right)
            .build();
        row.append(&scale);
        parent.append(&row);
        scale
    }

    /// Get the top-level widget for embedding.
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// The pitch slider is only usable while a pitch-shifted preset
    /// is selected AND the available shifter exposes a live semitone
    /// control (am_pitchshift/rubberband — the pitchScale fallback
    /// takes a ratio, not semitones).
    fn update_pitch_sensitivity(&self) {
        let preset = self.settings.borrow().audio_config.mic_preset;
        let ok = preset.default_pitch().is_some()
            && micfx::effect_control(MicEffect::Voice, preset).is_some();
        self.pitch_scale.set_sensitive(ok);
    }

    /// Re-apply persisted state to widgets and backend. Call once after
    /// the backend connected; spawns the daemon when needed.
    pub fn restore(&self) {
        let cfg = self.settings.borrow().audio_config.clone();
        self.updating.set(true);
        self.enable_check.set_active(cfg.mic_fx_enabled);
        self.gate_check.set_active(cfg.mic_gate);
        self.gate_scale.set_value(f64::from(cfg.mic_gate_level));
        self.pitch_scale.set_value(f64::from(cfg.mic_voice_pitch));
        self.update_pitch_sensitivity();
        self.echo_scale.set_value(f64::from(cfg.mic_echo_level));
        for (preset, check) in &self.preset_checks {
            check.set_active(*preset == cfg.mic_preset);
        }
        self.updating.set(false);
        self.fx_box.set_sensitive(cfg.mic_fx_enabled);

        if cfg.mic_fx_enabled || cfg.mic_gate {
            spawn_daemon(&self.last_spawn, &self.status);
        }
    }

    /// Route backend events: chain presence and mic level.
    pub fn apply_event(&self, event: &BackendEvent) {
        match event {
            BackendEvent::MicFxStateChanged(enabled) => {
                debug!("Mic fx chain state: {enabled}");
                // Presence drives control sensitivity + status only —
                // never the master checkbox (it is pure user intent;
                // a chain rebuild would otherwise flip it off). The
                // controls also stay locked when only the gate runs
                // the chain (denoise works without the fun effects).
                let intent = self.settings.borrow().audio_config.mic_fx_enabled;
                self.fx_box.set_sensitive(*enabled && intent);
                self.status.set_label(if *enabled {
                    "Virtuelles Mikrofon „Skald Mikrofon“ aktiv"
                } else {
                    ""
                });
                // Intent says chain should exist but it vanished — the
                // daemon probably died; try to bring it back.
                let want = {
                    let c = &self.settings.borrow().audio_config;
                    c.mic_fx_enabled || c.mic_gate
                };
                if !*enabled && want {
                    spawn_daemon(&self.last_spawn, &self.status);
                }
            }
            BackendEvent::MicLevel(peak) => {
                self.level_bar.set_value(f64::from(*peak));
            }
            _ => {}
        }
    }

    /// Wire widget signals.
    fn connect_signals(
        &self,
        backend: Rc<PwBackend>,
        persist: Rc<ConfigPersistence>,
        monitor_button: ToggleButton,
    ) {
        let pending = pending_slot();

        // Master toggle: persist intent, then let the daemon apply it.
        {
            let settings = Rc::clone(&self.settings);
            let persist = persist.clone();
            let updating = Rc::clone(&self.updating);
            let last_spawn = Rc::clone(&self.last_spawn);
            let status = self.status.clone();
            let box_ = self.fx_box.clone();
            self.enable_check.connect_toggled(move |check| {
                if updating.get() {
                    return;
                }
                let enabled = check.is_active();
                settings.borrow_mut().audio_config.mic_fx_enabled = enabled;
                save_now(&settings, &persist);
                // Immediate visual feedback; presence confirms shortly.
                box_.set_sensitive(enabled);
                if enabled {
                    spawn_daemon(&last_spawn, &status);
                }
            });
        }

        // Preset radio group → persist; the daemon's file watch
        // rebuilds the voice stage. A pitch preset also snaps the
        // pitch slider back to its base value — manual retuning is
        // per-session, each preset starts at its intended shift.
        for (preset, check) in &self.preset_checks {
            let preset = *preset;
            let settings = Rc::clone(&self.settings);
            let updating = Rc::clone(&self.updating);
            let pitch_scale = self.pitch_scale.clone();
            let save = debounced_save(
                Rc::clone(&self.settings),
                persist.clone(),
                Rc::clone(&pending),
            );
            check.connect_toggled(move |check| {
                if updating.get() || !check.is_active() {
                    return;
                }
                settings.borrow_mut().audio_config.mic_preset = preset;
                if let Some(base) = preset.default_pitch() {
                    settings.borrow_mut().audio_config.mic_voice_pitch = base;
                    pitch_scale.set_value(f64::from(base));
                }
                pitch_scale.set_sensitive(
                    preset.default_pitch().is_some()
                        && micfx::effect_control(MicEffect::Voice, preset).is_some(),
                );
                save();
            });
        }

        // Noise gate toggle → persist + spawn daemon (the gate alone
        // already runs the chain, no effects master needed).
        {
            let settings = Rc::clone(&self.settings);
            let updating = Rc::clone(&self.updating);
            let last_spawn = Rc::clone(&self.last_spawn);
            let status = self.status.clone();
            let save = debounced_save(
                Rc::clone(&self.settings),
                persist.clone(),
                Rc::clone(&pending),
            );
            self.gate_check.connect_toggled(move |check| {
                if updating.get() {
                    return;
                }
                let on = check.is_active();
                settings.borrow_mut().audio_config.mic_gate = on;
                save();
                if on {
                    spawn_daemon(&last_spawn, &status);
                }
            });
        }

        // Gate threshold + pitch + echo level: live params AND persist.
        type Setter = fn(&mut crate::audio::AudioConfig, f32);
        for (scale, effect, set) in [
            (
                &self.gate_scale,
                MicEffect::Denoise,
                (|c: &mut crate::audio::AudioConfig, v| c.mic_gate_level = v) as Setter,
            ),
            (
                &self.pitch_scale,
                MicEffect::Voice,
                |c: &mut crate::audio::AudioConfig, v| c.mic_voice_pitch = v,
            ),
            (
                &self.echo_scale,
                MicEffect::Echo,
                |c: &mut crate::audio::AudioConfig, v| c.mic_echo_level = v,
            ),
        ] {
            let backend = Rc::clone(&backend);
            let settings = Rc::clone(&self.settings);
            let updating = Rc::clone(&self.updating);
            let save = debounced_save(
                Rc::clone(&self.settings),
                persist.clone(),
                Rc::clone(&pending),
            );
            scale.connect_value_changed(move |scale| {
                if updating.get() {
                    return;
                }
                let v = scale.value() as f32;
                backend.set_mic_effect_value(effect, v);
                set(&mut settings.borrow_mut().audio_config, v);
                save();
            });
        }

        // Monitor test button → GUI-owned loopback module.
        monitor_button.connect_toggled(move |btn| {
            backend.set_mic_monitor(btn.is_active());
        });
    }
}
