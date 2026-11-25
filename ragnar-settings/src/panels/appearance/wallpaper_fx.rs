// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — livewall effects editor dialog
//
// Mini animated preview (fx_preview.rs runs the same GLSL file
// the daemon ships) plus one collapsible row per effect (glow,
// stars, mist, snow, rain, fireflies, cloud shadows — see
// wallpaper_fx_row::EFFECTS). "Region" arms the drag: each drag
// on the preview adds a region — up to four per effect, glow
// keeps a single band — and auto-enables the effect. "−"
// removes the last region, "Reset" restores defaults. Saving
// writes ~/.config/ragnar-settings/livewall-effects.ini —
// ragnar-wallpaper watches that file and respawns
// ragnar-livewall, so the real desktop picks it up instantly.

use gettextrs::gettext;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::fx_preview::{FxPreview, FxView, FX_GLOW};
use super::wallpaper_fx_ini as fx_ini;
use super::wallpaper_fx_row as fx_row;
use super::wallpaper_fx_row::EffectRow;

/// Open the effects editor for `image` (the wallpaper the
/// preview shows and the ini section is named after).
pub fn open(parent: &gtk4::Window, image: &Path) {
    let Ok(tex) = gtk4::gdk::Texture::from_filename(image) else {
        return;
    };
    let preview = Rc::new(FxPreview::new(&tex));
    let fx = preview.fx.clone();

    let win = gtk4::Window::builder()
        .title(gettext("Wallpaper effects"))
        .transient_for(parent)
        .modal(true)
        .default_width(560)
        .build();

    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(12);
    page.set_margin_end(12);
    page.append(&preview.area);
    page.append(&gtk4::Label::new(Some(&gettext(
        "Expand an effect, activate \"Region\", then drag on the \
         preview to add a region (glow keeps a single band).",
    ))));

    // u_edit id of the effect whose Region toggle is armed.
    let target = Rc::new(RefCell::new(0u8));

    let rows = Rc::new(
        fx_row::EFFECTS
            .iter()
            .map(|spec| fx_row::build(spec, &page))
            .collect::<Vec<EffectRow>>(),
    );
    for row in rows.iter() {
        fx_row::wire_switch(row, &fx);
        fx_row::wire_minus(row, &fx);
        fx_row::wire_reset(row, &fx);
        fx_row::wire_sliders(row, &fx);
    }
    wire_region_toggles(&rows, &target);
    let switches: Vec<(u8, gtk4::Switch)> = rows
        .iter()
        .map(|r| (r.spec.id, r.sw.clone()))
        .collect();
    wire_drag(&preview, &fx, &target, &switches);
    fx_ini::load_ini(image, &fx, &rows);

    // Buttons
    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    let save = gtk4::Button::with_label(&gettext("Save"));
    save.add_css_class("suggested-action");
    {
        let (w, f, img, rs) =
            (win.clone(), fx.clone(), image.to_path_buf(), rows.clone());
        save.connect_clicked(move |_| {
            fx_ini::save_ini(&img, &f, &rs);
            w.close();
        });
    }
    btns.append(&cancel);
    btns.append(&save);
    page.append(&btns);

    // The window holds the only strong preview ref; the clocks
    // run on Weaks and stop once the window is gone.
    {
        let keep = preview.clone();
        win.connect_close_request(move |_| {
            let _ = &keep;
            gtk4::glib::Propagation::Proceed
        });
    }
    {
        let pv = Rc::downgrade(&preview);
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let Some(p) = pv.upgrade() else {
                return gtk4::glib::ControlFlow::Break;
            };
            p.tick();
            gtk4::glib::ControlFlow::Continue
        });
    }
    // Sync clock: fx.edit follows the armed region toggle and
    // the count labels track configured rects (glow shows "1"
    // while its band exists).
    {
        let pv = Rc::downgrade(&preview);
        let (fx2, tgt2, rs) = (fx.clone(), target.clone(), rows.clone());
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
            if pv.upgrade().is_none() {
                return gtk4::glib::ControlFlow::Break;
            }
            let f = fx2.borrow();
            for row in rs.iter() {
                let n = if row.spec.id == FX_GLOW {
                    usize::from(f.aurora_rect[2] > 0.0)
                } else {
                    f.region_rects(row.spec.id)
                        .map(|r| fx_ini::region_count(r) as usize)
                        .unwrap_or(0)
                };
                let txt = if n == 0 { String::new() } else { n.to_string() };
                row.count.set_label(&txt);
            }
            drop(f);
            fx2.borrow_mut().edit = *tgt2.borrow() as f32;
            gtk4::glib::ControlFlow::Continue
        });
    }

    win.set_child(Some(&page));
    win.present();
}

/// Exclusive region toggles: arming one disarms the others and
/// points the drag target at its effect.
fn wire_region_toggles(rows: &Rc<Vec<EffectRow>>, target: &Rc<RefCell<u8>>) {
    for row in rows.iter() {
        let (rs, tgt) = (rows.clone(), target.clone());
        let id = row.spec.id;
        row.reg.connect_toggled(move |b| {
            if b.is_active() {
                for other in rs.iter() {
                    if other.spec.id != id {
                        other.reg.set_active(false);
                    }
                }
                *tgt.borrow_mut() = id;
            } else if *tgt.borrow() == id {
                *tgt.borrow_mut() = 0;
            }
        });
    }
}

/// Drag on the preview: glow replaces its single band; every
/// other effect fills one rect slot picked at drag_begin and
/// reused for the whole gesture (otherwise every motion event
/// would eat another slot). Drawing a region while the effect
/// is off switches it on.
fn wire_drag(
    preview: &Rc<FxPreview>,
    fx: &Rc<RefCell<FxView>>,
    target: &Rc<RefCell<u8>>,
    switches: &[(u8, gtk4::Switch)],
) {
    let drag = gtk4::GestureDrag::new();
    let start = Rc::new(RefCell::new((0.0f64, 0.0f64)));
    let slot = Rc::new(RefCell::new(0usize));
    {
        let (st, sl) = (start.clone(), slot.clone());
        let (fx, tgt) = (fx.clone(), target.clone());
        let switches = switches.to_vec();
        drag.connect_drag_begin(move |_, x, y| {
            *st.borrow_mut() = (x, y);
            let id = *tgt.borrow();
            if id == FX_GLOW || id == 0 {
                return;
            }
            // Slot first, then drop the borrow — set_active fires
            // state_set, which borrows the same RefCell.
            let pos = fx
                .borrow()
                .region_rects(id)
                .and_then(|rs| rs.iter().position(|r| r[2] == 0.0))
                .unwrap_or(3);
            *sl.borrow_mut() = pos;
            if let Some(sw) = switches.iter().find(|(i, _)| *i == id).map(|(_, s)| s) {
                if !sw.is_active() {
                    sw.set_active(true);
                }
            }
        });
    }
    {
        let (fx, tgt, st, sl) =
            (fx.clone(), target.clone(), start.clone(), slot.clone());
        let area = preview.area.clone();
        let switches = switches.to_vec();
        drag.connect_drag_update(move |_, dx, dy| {
            let id = *tgt.borrow();
            if id == 0 {
                return;
            }
            let (sx, sy) = *st.borrow();
            let w = area.width().max(1) as f64;
            let h = area.height().max(1) as f64;
            let (x0, y0, x1, y1) = (sx / w, sy / h, (sx + dx) / w, (sy + dy) / h);
            let r = [
                x0.min(x1).clamp(0.0, 1.0) as f32,
                y0.min(y1).clamp(0.0, 1.0) as f32,
                (x1 - x0).abs().clamp(0.01, 1.0) as f32,
                (y1 - y0).abs().clamp(0.01, 1.0) as f32,
            ];
            let mut f = fx.borrow_mut();
            if id == FX_GLOW {
                f.aurora_rect = r;
            } else if let Some((rects, p)) = f.region_fx(id) {
                rects[*sl.borrow()] = r;
                let on = switches
                    .iter()
                    .find(|(i, _)| *i == id)
                    .is_some_and(|(_, s)| s.is_active());
                p[2] = if on { fx_ini::region_count(rects) } else { 0.0 };
            }
        });
    }
    preview.area.add_controller(drag);
}
