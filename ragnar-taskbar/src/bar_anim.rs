// Animated gradient background for a bar — a GtkDrawingArea layered
// UNDER the applet strip inside a GtkOverlay. The user's colour list
// flows along the configured direction; the phase lives in BarState
// so layout rebuilds keep the animation seamless instead of
// restarting the gradient.
// Cost discipline: one timer per animated bar at the configured fps
// (default 30), each tick only bumps the phase and queues a redraw;
// painting is a single Cairo linear-gradient fill of the bar area.
// Disabled animation installs nothing — zero timers, zero redraws.
use crate::bar::State;
use gtk4::prelude::*;
use std::time::Duration;

/// Create the animated background layer and start its phase timer.
/// Only called while `cfg.anim` is on — rebuild_root replaces the
/// whole layer on every commit, and the old layer's timer dies with
/// the widget via its weak reference.
pub fn paint_layer(state: &State) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_content_width(1);
    area.set_content_height(1);
    {
        let st = state.clone();
        area.set_draw_func(move |_, cr, w, h| paint(&st, cr, w as f64, h as f64));
    }
    let fps = state.borrow().cfg.anim_fps.clamp(5, 120) as u64;
    let (weak, st) = (area.downgrade(), state.clone());
    gtk4::glib::timeout_add_local(Duration::from_millis(1000 / fps), move || {
        let Some(a) = weak.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };
        let s = st.borrow();
        // Bar removed or the user switched the animation off — die
        // instead of repainting a destroyed or static bar.
        if s.dead || !s.cfg.anim {
            return gtk4::glib::ControlFlow::Break;
        }
        let secs = 1.0 / s.cfg.anim_fps.clamp(5, 120) as f64;
        // Full cycle = `13 - speed` seconds (speed 1..10 → 12s..3s).
        let cycle = (13.0 - s.cfg.anim_speed.clamp(1, 10) as f64).max(2.0);
        s.anim_phase.set(s.anim_phase.get() + secs / cycle);
        drop(s);
        a.queue_draw();
        gtk4::glib::ControlFlow::Continue
    });
    area
}

/// Paint the flowing gradient. The colour ring is laid out along the
/// gradient axis as cyclic stops; shifting their parameter by the
/// phase moves the whole pattern — stops outside [0,1] are legal and
/// keep the edges continuously covered.
fn paint(state: &State, cr: &gtk4::cairo::Context, w: f64, h: f64) {
    let st = state.borrow();
    let cfg = &st.cfg;
    let cols: Vec<gtk4::gdk::RGBA> = cfg
        .anim_colors
        .iter()
        .filter_map(|s| gtk4::gdk::RGBA::parse(s).ok())
        .collect();
    if cols.len() < 2 {
        return;
    }
    let n = cols.len() as i32;
    let phase = st.anim_phase.get().fract().abs();
    let alpha = cfg.opacity.clamp(10, 100) as f64 / 100.0;

    // Stops at s = k/n - phase drift toward the axis START, so the
    // axis begins at the edge the colours should flow towards.
    let (x0, y0, x1, y1, drift) = match cfg.anim_dir.as_str() {
        "ltr" => (w, 0.0, 0.0, 0.0, true),
        "rtl" => (0.0, 0.0, w, 0.0, true),
        "down" => (0.0, h, 0.0, 0.0, true),
        "up" => (0.0, 0.0, 0.0, h, true),
        // "spin" — a linear gradient rotated around the centre.
        _ => {
            let ang = phase * std::f64::consts::TAU;
            let r = (w * w + h * h).sqrt() / 2.0;
            let (cx, cy) = (w / 2.0, h / 2.0);
            (
                cx - r * ang.cos(),
                cy - r * ang.sin(),
                cx + r * ang.cos(),
                cy + r * ang.sin(),
                false,
            )
        }
    };
    let grad = gtk4::cairo::LinearGradient::new(x0, y0, x1, y1);
    if drift {
        // -1 ..= 2n covers [0,1] plus margin on both ends at any phase.
        for k in -1..=(2 * n) {
            let c = &cols[k.rem_euclid(n) as usize];
            grad.add_color_stop_rgba(
                k as f64 / n as f64 - phase,
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                alpha,
            );
        }
    } else {
        // Spin: fixed cyclic stops, only the axis rotates — closing
        // the ring with the first colour keeps the wrap seamless.
        for k in 0..=n {
            let c = &cols[(k % n) as usize];
            grad.add_color_stop_rgba(
                k as f64 / n as f64,
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                alpha,
            );
        }
    }
    let _ = cr.set_source(&grad);
    let _ = cr.paint();
}
