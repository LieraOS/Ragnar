// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — effect row specs and builders
//
// One collapsible Expander row per effect: the header carries
// name + enable switch; unfolding reveals the region controls
// (Region toggle, "−", Reset, count) and the effect's parameter
// sliders. All effects are table-driven from EFFECTS so adding
// another shader block is a one-line spec.

use gettextrs::gettext;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::fx_preview::{FxView, FX_CLOUDS, FX_FLIES, FX_GLOW, FX_MIST, FX_RAIN, FX_SNOW, FX_STARS};
use super::wallpaper_fx_ini as fx_ini;

/// One parameter slider of an effect.
pub struct SliderSpec {
    pub label: &'static str,
    /// ini key suffix, e.g. "density" -> "stars.density".
    pub ini: &'static str,
    pub lo: f64,
    pub hi: f64,
    pub def: f32,
    /// FxView param index — 0/1 for every effect.
    pub param: usize,
}

/// Static description of one shader effect.
pub struct EffectSpec {
    /// Shader u_edit id (1 glow .. 7 clouds).
    pub id: u8,
    pub name: &'static str,
    /// ini key prefix, e.g. "stars".
    pub ini: &'static str,
    /// Reset target for the enable switch.
    pub enabled_def: bool,
    pub sliders: &'static [SliderSpec],
}

pub const EFFECTS: &[EffectSpec] = &[
    EffectSpec {
        id: FX_GLOW,
        name: "Glow",
        ini: "aurora",
        enabled_def: true,
        sliders: &[SliderSpec {
            label: "Glow intensity",
            ini: "intensity",
            lo: 0.0,
            hi: 2.0,
            def: 1.0,
            param: 0,
        }],
    },
    EffectSpec {
        id: FX_STARS,
        name: "Stars",
        ini: "stars",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Star density", ini: "density", lo: 0.0, hi: 1.0, def: 0.7, param: 0 },
            SliderSpec { label: "Star twinkle speed", ini: "speed", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
    EffectSpec {
        id: FX_MIST,
        name: "Mist",
        ini: "mist",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Mist opacity", ini: "opacity", lo: 0.0, hi: 1.0, def: 0.35, param: 0 },
            SliderSpec { label: "Mist drift speed", ini: "drift", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
    EffectSpec {
        id: FX_SNOW,
        name: "Snow",
        ini: "snow",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Snow density", ini: "density", lo: 0.0, hi: 1.0, def: 0.7, param: 0 },
            SliderSpec { label: "Snow fall speed", ini: "speed", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
    EffectSpec {
        id: FX_RAIN,
        name: "Rain",
        ini: "rain",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Rain strength", ini: "strength", lo: 0.0, hi: 1.0, def: 0.6, param: 0 },
            SliderSpec { label: "Rain speed", ini: "speed", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
    EffectSpec {
        id: FX_FLIES,
        name: "Fireflies",
        ini: "fireflies",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Firefly count", ini: "density", lo: 0.0, hi: 1.0, def: 0.6, param: 0 },
            SliderSpec { label: "Firefly drift speed", ini: "speed", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
    EffectSpec {
        id: FX_CLOUDS,
        name: "Cloud shadows",
        ini: "clouds",
        enabled_def: false,
        sliders: &[
            SliderSpec { label: "Cloud shadow opacity", ini: "opacity", lo: 0.0, hi: 1.0, def: 0.4, param: 0 },
            SliderSpec { label: "Cloud shadow speed", ini: "speed", lo: 0.0, hi: 3.0, def: 1.0, param: 1 },
        ],
    },
];

/// Widgets of one effect row — everything the dialog syncs.
pub struct EffectRow {
    pub spec: &'static EffectSpec,
    pub sw: gtk4::Switch,
    pub reg: gtk4::ToggleButton,
    pub minus: gtk4::Button,
    pub reset: gtk4::Button,
    pub count: gtk4::Label,
    pub scales: Vec<gtk4::Scale>,
}

/// Build the collapsible row for one effect.
pub fn build(spec: &'static EffectSpec, page: &gtk4::Box) -> EffectRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    let name = gtk4::Label::new(Some(&gettext(spec.name)));
    name.set_halign(gtk4::Align::Start);
    name.set_hexpand(true);
    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    head.append(&name);
    head.append(&sw);

    let reg = gtk4::ToggleButton::with_label(&gettext("Region"));
    let minus = gtk4::Button::with_label("−");
    minus.set_tooltip_text(Some(&gettext("Remove last region")));
    let reset = gtk4::Button::with_label(&gettext("Reset"));
    reset.set_tooltip_text(Some(&gettext("Restore this effect's defaults")));
    let count = gtk4::Label::new(None);
    count.set_width_chars(3);
    let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    controls.append(&reg);
    controls.append(&minus);
    controls.append(&reset);
    controls.append(&count);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_start(24);
    body.append(&controls);
    let mut scales = Vec::new();
    for s in spec.sliders {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let l = gtk4::Label::new(Some(&gettext(s.label)));
        l.set_width_chars(18);
        l.set_halign(gtk4::Align::Start);
        row.append(&l);
        let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, s.lo, s.hi, 0.05);
        scale.set_hexpand(true);
        scale.set_draw_value(true);
        row.append(&scale);
        body.append(&row);
        scales.push(scale);
    }

    let exp = gtk4::Expander::new(None);
    exp.set_label_widget(Some(&head));
    exp.set_child(Some(&body));
    page.append(&exp);
    EffectRow { spec, sw, reg, minus, reset, count, scales }
}

/// Enable switch -> FxView. Region effects write their enabled
/// region count, the glow its on/off flag.
pub fn wire_switch(row: &EffectRow, fx: &Rc<RefCell<FxView>>) {
    let f = fx.clone();
    let id = row.spec.id;
    row.sw.connect_state_set(move |_, on| {
        let mut fv = f.borrow_mut();
        if id == FX_GLOW {
            fv.aurora[1] = if on { 1.0 } else { 0.0 };
        } else if let Some((rects, p)) = fv.region_fx(id) {
            p[2] = if on { fx_ini::region_count(rects) } else { 0.0 };
        }
        gtk4::glib::Propagation::Proceed
    });
}

/// "−" removes the last configured region; glow resets to its
/// full band. The enabled count follows the switch.
pub fn wire_minus(row: &EffectRow, fx: &Rc<RefCell<FxView>>) {
    let (f, sw) = (fx.clone(), row.sw.clone());
    let id = row.spec.id;
    row.minus.connect_clicked(move |_| {
        let mut fv = f.borrow_mut();
        if id == FX_GLOW {
            fv.aurora_rect = [0.0, 0.0, 1.0, 1.0];
        } else if let Some((rects, p)) = fv.region_fx(id) {
            let cnt = fx_ini::region_count(rects) as usize;
            if cnt > 0 {
                rects[cnt - 1] = [0.0; 4];
            }
            p[2] = if sw.is_active() { fx_ini::region_count(rects) } else { 0.0 };
        }
    });
}

/// "Reset" restores the spec defaults: fx values first, then the
/// widgets (set_active/set_value fire handlers that borrow the
/// same RefCell — the borrow must be dropped before).
pub fn wire_reset(row: &EffectRow, fx: &Rc<RefCell<FxView>>) {
    let (f, sw) = (fx.clone(), row.sw.clone());
    let scales: Vec<gtk4::Scale> = row.scales.clone();
    let spec = row.spec;
    row.reset.connect_clicked(move |_| {
        {
            let mut fv = f.borrow_mut();
            if spec.id == FX_GLOW {
                fv.aurora_rect = [0.0, 0.0, 1.0, 1.0];
                fv.aurora = [spec.sliders[0].def, 1.0];
            } else if let Some((rects, p)) = fv.region_fx(spec.id) {
                *rects = [[0.0; 4]; 4];
                *p = [spec.sliders[0].def, spec.sliders[1].def, 0.0];
            }
        }
        sw.set_active(spec.enabled_def);
        for (s, sc) in spec.sliders.iter().zip(&scales) {
            sc.set_value(s.def as f64);
        }
    });
}

/// Parameter sliders push straight into the FxView.
pub fn wire_sliders(row: &EffectRow, fx: &Rc<RefCell<FxView>>) {
    for (s, sc) in spec_sliders(row).iter().zip(row.scales.iter()) {
        let (f, id, param) = (fx.clone(), row.spec.id, s.param);
        sc.connect_value_changed(move |sl| {
            *f.borrow_mut().param_mut(id, param) = sl.value() as f32;
        });
    }
}

fn spec_sliders(row: &EffectRow) -> &'static [SliderSpec] {
    row.spec.sliders
}
