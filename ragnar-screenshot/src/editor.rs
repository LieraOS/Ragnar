// SPDX-License-Identifier: GPL-3.0-or-later
// In-place image editor for the captured frame: the user drags
// rectangles over the preview — either a yellow highlight or a
// solid black redaction bar. Applied edits bake straight into the
// pixel buffer, so Save writes the edited picture.

use crate::model::CapturedImage;
use gtk4::cairo;
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// One user-drawn rectangle in image pixel space.
#[derive(Clone, Copy)]
struct Mark {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    black: bool,
}

/// The editor widget + its callbacks into the app.
pub struct Editor {
    pub widget: gtk4::Box,
}

impl Editor {
    /// Build the editor page for `img`. `on_done` receives the baked
    /// image (or None on cancel).
    pub fn new(img: &CapturedImage, on_done: impl Fn(Option<CapturedImage>) + 'static) -> Self {
        let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        let marks = Rc::new(RefCell::new(Vec::<Mark>::new()));
        let drag: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let tool_black = Rc::new(Cell::new(true));
        let img_dims = Rc::new(RefCell::new((img.width as i32, img.height as i32)));
        let fit = Rc::new(Cell::new((1.0_f64, 0.0_f64, 0.0_f64)));

        // Toolbar: tool toggle + undo + done + cancel.
        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        bar.set_halign(gtk4::Align::Center);
        let t_black = gtk4::ToggleButton::with_label("Schwärzen");
        t_black.set_active(true);
        let t_high = gtk4::ToggleButton::with_label("Markieren");
        t_high.set_group(Some(&t_black));
        let undo = gtk4::Button::with_label("Rückgängig");
        let done = gtk4::Button::with_label("Fertig");
        done.add_css_class("suggested-action");
        let cancel = gtk4::Button::with_label("Abbrechen");
        for b in [&t_black, &t_high] {
            bar.append(b.upcast_ref::<gtk4::Widget>());
        }
        bar.append(&undo);
        bar.append(&done);
        bar.append(&cancel);
        widget.append(&bar);

        let area = gtk4::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_can_focus(true);
        widget.append(&area);

        // Texture from pixels, painted in draw via a second pass —
        // snapshot() with explicit dest rect keeps it simple:
        // we render the texture into a snapshot in draw() above…
        // …reality: gtk Texture has no cairo paint. Convert to a
        // cairo ImageSurface once — cheapest correct path.
        let surface = pixels_to_surface(img);
        {
            let dims = img_dims.clone();
            let marks2 = marks.clone();
            let drag2 = drag.clone();
            let black2 = tool_black.clone();
            let fit2 = fit.clone();
            let surf = surface.clone();
            area.set_draw_func(move |_, cr, w, h| {
                let (iw, ih) = *dims.borrow();
                let s = (w as f64 / iw as f64).min(h as f64 / ih as f64);
                let ox = (w as f64 - iw as f64 * s) / 2.0;
                let oy = (h as f64 - ih as f64 * s) / 2.0;
                fit2.set((s, ox, oy));
                let _ = cr.save();
                cr.translate(ox, oy);
                cr.scale(s, s);
                let _ = cr.set_source_surface(&surf, 0.0, 0.0);
                let _ = cr.paint();
                let _ = cr.restore();
                for m in marks2.borrow().iter() {
                    draw_mark(cr, *m, s, ox, oy);
                }
                if let Some((sx, sy)) = *drag2.borrow() {
                    if let Some((cx, cy)) = CURRENT.with(|c| *c.borrow()) {
                        draw_mark(
                            cr,
                            Mark {
                                x: sx.min(cx),
                                y: sy.min(cy),
                                w: (cx - sx).abs(),
                                h: (cy - sy).abs(),
                                black: black2.get(),
                            },
                            s,
                            ox,
                            oy,
                        );
                    }
                }
            });
        }

        let gesture = gtk4::GestureDrag::new();
        {
            let drag = drag.clone();
            let fit = fit.clone();
            gesture.connect_drag_begin(move |_, x, y| {
                *drag.borrow_mut() = Some(img_coords(x, y, fit.get()));
            });
        }
        {
            let drag = drag.clone();
            let fit = fit.clone();
            let a = area.clone();
            gesture.connect_drag_update(move |_, ox, oy| {
                if let Some((sx, sy)) = *drag.borrow() {
                    let _ = (sx, sy);
                    CURRENT.with(|c| {
                        *c.borrow_mut() = Some(img_coords(
                            sx * fit.get().0 + fit.get().1 + ox,
                            sy * fit.get().0 + fit.get().2 + oy,
                            fit.get(),
                        ));
                    });
                }
                a.queue_draw();
            });
        }
        {
            let drag = drag.clone();
            let marks = marks.clone();
            let black = tool_black.clone();
            let a = area.clone();
            gesture.connect_drag_end(move |_, _, _| {
                if let (Some((sx, sy)), Some((cx, cy))) =
                    (*drag.borrow(), CURRENT.with(|c| *c.borrow()))
                {
                    let (w, h) = ((cx - sx).abs(), (cy - sy).abs());
                    if w > 4.0 && h > 4.0 {
                        marks.borrow_mut().push(Mark {
                            x: sx.min(cx),
                            y: sy.min(cy),
                            w,
                            h,
                            black: black.get(),
                        });
                    }
                }
                *drag.borrow_mut() = None;
                CURRENT.with(|c| *c.borrow_mut() = None);
                a.queue_draw();
            });
        }
        area.add_controller(gesture);

        // Wire the toolbar.
        {
            let black = tool_black.clone();
            t_black.connect_toggled(move |b| black.set(b.is_active()));
        }
        {
            let black = tool_black.clone();
            t_high.connect_toggled(move |b| {
                if b.is_active() {
                    black.set(false);
                }
            });
        }
        {
            let marks = marks.clone();
            let a = area.clone();
            undo.connect_clicked(move |_| {
                marks.borrow_mut().pop();
                a.queue_draw();
            });
        }
        let on_done = Rc::new(on_done);
        {
            let marks = marks.clone();
            let done_cb = on_done.clone();
            let iw = img.width;
            let ih = img.height;
            let px = img.pixels.clone();
            done.connect_clicked(move |_| {
                done_cb(Some(bake(&px, iw, ih, &marks.borrow())));
            });
        }
        {
            let cancel_cb = on_done.clone();
            cancel.connect_clicked(move |_| cancel_cb(None));
        }

        Self { widget }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<(f64, f64)>> = const { RefCell::new(None) };
}

/// Widget coordinates → image pixel coordinates via the fit
/// transform recorded during the last draw.
fn img_coords(x: f64, y: f64, fit: (f64, f64, f64)) -> (f64, f64) {
    let (s, ox, oy) = fit;
    ((x - ox) / s, (y - oy) / s)
}

/// Paint one mark — black bar or translucent yellow highlight.
fn draw_mark(cr: &cairo::Context, m: Mark, s: f64, ox: f64, oy: f64) {
    let (x, y) = (ox + m.x * s, oy + m.y * s);
    let (w, h) = (m.w * s, m.h * s);
    if m.black {
        cr.set_source_rgb(0.0, 0.0, 0.0);
        cr.rectangle(x, y, w, h);
        let _ = cr.fill();
    } else {
        cr.set_source_rgba(1.0, 0.9, 0.0, 0.35);
        cr.rectangle(x, y, w, h);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(1.0, 0.85, 0.0, 0.9);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
    }
}

/// Bake every mark into the pixel buffer (BGRA in place clone).
fn bake(px: &[u8], w: usize, h: usize, marks: &[Mark]) -> CapturedImage {
    let mut out = px.to_vec();
    for m in marks {
        let x0 = (m.x as i64).clamp(0, w as i64);
        let y0 = (m.y as i64).clamp(0, h as i64);
        let x1 = ((m.x + m.w) as i64).clamp(0, w as i64);
        let y1 = ((m.y + m.h) as i64).clamp(0, h as i64);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = (y as usize * w + x as usize) * 4;
                if m.black {
                    out[d] = 0;
                    out[d + 1] = 0;
                    out[d + 2] = 0;
                    out[d + 3] = 255;
                } else {
                    // 35% yellow over the pixel.
                    for (i, yv) in [(0usize, 0u8), (1, 235), (2, 255)] {
                        out[d + i] = ((out[d + i] as u32 * 65 + yv as u32 * 35) / 100) as u8;
                    }
                }
            }
        }
    }
    CapturedImage {
        width: w,
        height: h,
        pixels: out,
    }
}

/// BGRA pixels → cairo surface for drawing.
fn pixels_to_surface(img: &CapturedImage) -> cairo::ImageSurface {
    let mut surf =
        cairo::ImageSurface::create(cairo::Format::ARgb32, img.width as i32, img.height as i32)
            .expect("cairo surface");
    let stride = surf.stride() as usize;
    {
        let mut data = surf.data().expect("surface data");
        for row in 0..img.height {
            for col in 0..img.width {
                let s = (row * img.width + col) * 4;
                let d = row * stride + col * 4;
                data[d] = img.pixels[s];
                data[d + 1] = img.pixels[s + 1];
                data[d + 2] = img.pixels[s + 2];
                data[d + 3] = img.pixels[s + 3];
            }
        }
    }
    surf
}
