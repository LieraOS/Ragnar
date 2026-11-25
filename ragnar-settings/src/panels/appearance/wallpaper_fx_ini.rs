// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — livewall effects ini round-trip
//
// load_ini/save_ini for the effects dialog: the ini keeps one
// section per wallpaper path, regions are ';'-separated rect
// lists. ini::write_key preserves other images' sections.
// Effect ids and defaults come from wallpaper_fx_row::EFFECTS.

use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::panels::ini;

use super::fx_preview::{FxView, FX_GLOW};
use super::wallpaper_fx_row::EffectRow;

/// Region count actually used by the shader.
pub fn region_count(rs: &[[f32; 4]; 4]) -> f32 {
    rs.iter().filter(|r| r[2] > 0.0 && r[3] > 0.0).count() as f32
}

/// Config file shared with ragnar-livewall.
pub fn ini_path() -> PathBuf {
    ini::config_path("ragnar-settings/livewall-effects.ini")
}

/// Read saved values back into widget + FxView state. Widget
/// writes happen only after every RefCell borrow is dropped —
/// set_active/set_value fire handlers that borrow it too.
pub fn load_ini(image: &Path, fx: &Rc<RefCell<FxView>>, rows: &[EffectRow]) {
    let map = ini::read(&ini_path());
    let sec = image.to_string_lossy().to_string();
    let getf = |k: &str, d: f32| {
        ini::get(&map, &sec, k)
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(d)
    };
    // (switch state, slider values) collected per row.
    let mut widget_state: Vec<(bool, Vec<f32>)> = Vec::new();
    {
        let mut f = fx.borrow_mut();
        for row in rows {
            let spec = row.spec;
            if spec.id == FX_GLOW {
                if let Some(v) = ini::get(&map, &sec, "aurora.region") {
                    if let Some(r) = parse_rects(&v).first() {
                        f.aurora_rect = *r;
                    }
                }
                f.aurora[0] = getf("aurora.intensity", 1.0);
                f.aurora[1] =
                    if ini::get(&map, &sec, "aurora").as_deref() != Some("false") {
                        1.0
                    } else {
                        0.0
                    };
                widget_state.push((f.aurora[1] > 0.5, vec![f.aurora[0]]));
                continue;
            }
            let Some((rects, p)) = f.region_fx(spec.id) else {
                continue;
            };
            if let Some(v) = ini::get(&map, &sec, &format!("{}.region", spec.ini)) {
                *rects = parse_rects(&v);
            }
            let on = ini::get(&map, &sec, spec.ini).as_deref() == Some("true");
            p[0] = getf(
                &format!("{}.{}", spec.ini, spec.sliders[0].ini),
                spec.sliders[0].def,
            );
            p[1] = getf(
                &format!("{}.{}", spec.ini, spec.sliders[1].ini),
                spec.sliders[1].def,
            );
            p[2] = if on { region_count(rects) } else { 0.0 };
            widget_state.push((on, vec![p[0], p[1]]));
        }
    }
    for (row, (on, vals)) in rows.iter().zip(widget_state) {
        row.sw.set_active(on);
        for (sc, v) in row.scales.iter().zip(vals) {
            sc.set_value(v as f64);
        }
    }
}

/// Write the section back — ini::write_key preserves any other
/// wallpaper's sections already in the file.
pub fn save_ini(image: &Path, fx: &Rc<RefCell<FxView>>, rows: &[EffectRow]) {
    let path = ini_path();
    let sec = image.to_string_lossy().to_string();
    let on = |b: bool| if b { "true".into() } else { "false".into() };
    let f = fx.borrow();
    for row in rows {
        let spec = row.spec;
        let mut kv: Vec<(String, String)> =
            vec![(spec.ini.into(), on(row.sw.is_active()))];
        if spec.id == FX_GLOW {
            kv.push(("aurora.region".into(), rect_str(&f.aurora_rect)));
        } else if let Some(rects) = f.region_rects(spec.id) {
            kv.push((format!("{}.region", spec.ini), rects_str(rects)));
        }
        for (s, sc) in spec.sliders.iter().zip(&row.scales) {
            kv.push((
                format!("{}.{}", spec.ini, s.ini),
                format!("{:.2}", sc.value()),
            ));
        }
        for (k, v) in kv {
            let _ = ini::write_key(&path, &sec, &k, &v);
        }
    }
}

fn rect_str(r: &[f32; 4]) -> String {
    format!("{:.3},{:.3},{:.3},{:.3}", r[0], r[1], r[2], r[3])
}

/// All configured regions joined by ';' — empty rects are
/// skipped (trailing slots below the count).
fn rects_str(rs: &[[f32; 4]; 4]) -> String {
    rs.iter()
        .filter(|r| r[2] > 0.0 && r[3] > 0.0)
        .map(rect_str)
        .collect::<Vec<_>>()
        .join(";")
}

/// Parse "x,y,w,h ; x,y,w,h ; …" — up to four rects.
fn parse_rects(v: &str) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (i, part) in v.split(';').take(4).enumerate() {
        for (j, n) in part.split(',').take(4).enumerate() {
            out[i][j] = n.trim().parse().unwrap_or(0.0);
        }
    }
    out
}
