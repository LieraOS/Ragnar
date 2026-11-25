// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Selection overlay: one fullscreen layer-shell window per output,
// dimmed everywhere except the dragged rectangle (the "veil" the
// user lifts off the picked area).

use gtk4::prelude::*;
use gtk4::{cairo, gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;

/// Result delivered when selection finishes.
/// (monitor_index, logical x, y, w, h inside that monitor)
pub type Selection = Option<(usize, i32, i32, i32, i32)>;

/// State per overlay window: dragged rect in logical output coords.
struct Sel {
    start: Option<(f64, f64)>,
    cur: Option<(f64, f64)>,
}

/// Show one dim+reveal overlay per monitor. `on_done` fires once —
/// with `Some(rect)` on confirm, `None` on Escape.
pub fn run(
    monitors: &[(i32, i32, i32, i32)], // logical x,y,w,h per output
    displays: &[gdk::Monitor],
    on_done: impl Fn(Selection) + 'static,
) {
    let on_done = Rc::new(on_done);
    let windows: Rc<RefCell<Vec<gtk4::Window>>> = Rc::new(RefCell::new(Vec::new()));

    for (idx, mon) in displays.iter().enumerate() {
        let win = gtk4::Window::new();
        // Transparent surface — GTK4 has no app_paintable, the
        // transparency comes from the CSS class loaded in ui.rs.
        // Without it the layer window would paint an opaque theme
        // background over the whole screen.
        win.set_decorated(false);
        win.add_css_class("ragnar-shot-overlay");
        win.init_layer_shell();
        win.set_layer(Layer::Overlay);
        for e in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            win.set_anchor(e, true);
        }
        win.set_exclusive_zone(-1);
        win.set_keyboard_mode(KeyboardMode::Exclusive);
        win.set_display(&mon.display());
        win.set_monitor(Some(mon));

        let state = Rc::new(RefCell::new(Sel {
            start: None,
            cur: None,
        }));
        let area = gtk4::DrawingArea::new();
        {
            let st = state.clone();
            area.set_draw_func(move |_, cr, w, h| {
                let (x, y) = (w as f64, h as f64);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
                cr.rectangle(0.0, 0.0, x, y);
                let s = st.borrow();
                if let (Some((sx, sy)), Some((cx, cy))) = (s.start, s.cur) {
                    let (rx, ry) = (sx.min(cx), sy.min(cy));
                    let (rw, rh) = ((cx - sx).abs(), (cy - sy).abs());
                    // Carve the clear region out of the dim rect.
                    cr.set_fill_rule(cairo::FillRule::EvenOdd);
                    cr.rectangle(rx, ry, rw, rh);
                }
                let _ = cr.fill();
                if let (Some((sx, sy)), Some((cx, cy))) = (s.start, s.cur) {
                    cr.set_fill_rule(cairo::FillRule::Winding);
                    cr.set_source_rgba(0.3, 0.6, 1.0, 0.9);
                    cr.set_line_width(1.5);
                    cr.rectangle(sx.min(cx), sy.min(cy), (cx - sx).abs(), (cy - sy).abs());
                    let _ = cr.stroke();
                }
            });
        }

        let drag = gtk4::GestureDrag::new();
        {
            let st = state.clone();
            let a = area.clone();
            drag.connect_drag_begin(move |_, x, y| {
                let mut s = st.borrow_mut();
                s.start = Some((x, y));
                s.cur = Some((x, y));
                a.queue_draw();
            });
        }
        {
            let st = state.clone();
            let a = area.clone();
            drag.connect_drag_update(move |_, ox, oy| {
                let mut s = st.borrow_mut();
                if let Some((sx, sy)) = s.start {
                    s.cur = Some((sx + ox, sy + oy));
                }
                a.queue_draw();
            });
        }
        {
            let st = state.clone();
            let a = area.clone();
            let wins = windows.clone();
            let done = on_done.clone();
            let (mw, mh) = (
                monitors.get(idx).map(|m| m.2).unwrap_or(0),
                monitors.get(idx).map(|m| m.3).unwrap_or(0),
            );
            drag.connect_drag_end(move |_, _, _| {
                let s = st.borrow();
                if let (Some((sx, sy)), Some((cx, cy))) = (s.start, s.cur) {
                    let (rx, ry) = (sx.min(cx) as i32, sy.min(cy) as i32);
                    let (rw, rh) = ((cx - sx).abs() as i32, (cy - sy).abs() as i32);
                    if rw > 4 && rh > 4 {
                        let r = (
                            idx,
                            rx.clamp(0, mw),
                            ry.clamp(0, mh),
                            rw.min(mw - rx),
                            rh.min(mh - ry),
                        );
                        for w in wins.borrow().iter() {
                            w.close();
                        }
                        done(Some(r));
                        return;
                    }
                }
                a.queue_draw();
            });
        }
        area.add_controller(drag);

        // Escape cancels everything.
        let key = gtk4::EventControllerKey::new();
        {
            let wins = windows.clone();
            let done = on_done.clone();
            key.connect_key_pressed(move |_, k, _, _| {
                if k == gdk::Key::Escape {
                    for w in wins.borrow().iter() {
                        w.close();
                    }
                    done(None);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }
        win.add_controller(key);
        win.set_child(Some(&area));
        win.present();
        area.grab_focus();
        windows.borrow_mut().push(win);
    }
}
