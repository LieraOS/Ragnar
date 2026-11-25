// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — the image canvas.
//
// GtkDrawingArea with wheel zoom, drag pan and fit modes. The
// RGBA buffer becomes a cairo ImageSurface once per image; every
// frame afterwards is a transform blit — zoom is free and the
// clamp sits at 25600%.

use crate::decode::Decoded;
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Fit behaviour on image change.
#[derive(Clone, Copy, PartialEq)]
pub enum FitMode {
    FitWindow,
    ActualSize,
}

/// Shared canvas state for the window's callbacks.
pub struct Canvas {
    pub area: gtk4::DrawingArea,
    surface: RefCell<Option<gtk4::cairo::ImageSurface>>,
    zoom: Cell<f64>,
    off_x: Cell<f64>,
    off_y: Cell<f64>,
    pub fit: Cell<FitMode>,
    img_w: Cell<f64>,
    img_h: Cell<f64>,
    drag_anchor: Cell<(f64, f64)>,
}

impl Canvas {
    pub fn new() -> Rc<Canvas> {
        let c = Rc::new(Canvas {
            area: gtk4::DrawingArea::new(),
            surface: RefCell::new(None),
            zoom: Cell::new(1.0),
            off_x: Cell::new(0.0),
            off_y: Cell::new(0.0),
            fit: Cell::new(FitMode::FitWindow),
            img_w: Cell::new(0.0),
            img_h: Cell::new(0.0),
            drag_anchor: Cell::new((0.0, 0.0)),
        });
        c.area.set_hexpand(true);
        c.area.set_vexpand(true);
        c.area.set_focusable(true);

        let st = c.clone();
        c.area.set_draw_func(move |_, cr, w, h| {
            cr.set_source_rgb(0.07, 0.07, 0.07);
            let _ = cr.paint();
            let borrow = st.surface.borrow();
            let Some(s) = borrow.as_ref() else { return };
            let (tw, th) = (s.width() as f64, s.height() as f64);
            let z = st.zoom.get();
            // Center + user pan offset.
            let ox = (w as f64 - tw * z) / 2.0 + st.off_x.get();
            let oy = (h as f64 - th * z) / 2.0 + st.off_y.get();
            cr.translate(ox, oy);
            cr.scale(z, z);
            let pattern = gtk4::cairo::SurfacePattern::create(s);
            // Nearest above 400% keeps pixels crisp;
            // bilinear below for smooth minification.
            pattern.set_filter(if z >= 4.0 {
                gtk4::cairo::Filter::Nearest
            } else {
                gtk4::cairo::Filter::Good
            });
            let _ = cr.set_source(&pattern);
            cr.rectangle(0.0, 0.0, tw, th);
            let _ = cr.fill();
        });

        // Scroll: zoom, clamped 1%..25600% — "endless" like the
        // original.
        let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        let st = c.clone();
        scroll.connect_scroll(move |_, _dx, dy| {
            let f = if dy < 0.0 { 1.15 } else { 1.0 / 1.15 };
            st.zoom.set((st.zoom.get() * f).clamp(0.01, 256.0));
            st.fit.set(FitMode::ActualSize);
            st.area.queue_draw();
            gtk4::glib::Propagation::Stop
        });
        c.area.add_controller(scroll);

        // Drag: pan by tracking the offset at drag start.
        let drag = gtk4::GestureDrag::new();
        let st = c.clone();
        drag.connect_drag_begin(move |_, _x, _y| {
            st.drag_anchor.set((st.off_x.get(), st.off_y.get()));
        });
        let st = c.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let (ox, oy) = st.drag_anchor.get();
            st.off_x.set(ox + dx);
            st.off_y.set(oy + dy);
            st.area.queue_draw();
        });
        c.area.add_controller(drag);
        c
    }

    /// Show a decoded image — swizzle RGBA → cairo ARGB32 once.
    pub fn show(&self, d: &Decoded) {
        // cairo ARGB32 is native-endian 0xAARRGGBB, i.e. BGRA byte
        // order on little-endian — swizzle once at upload time.
        let mut bgra = Vec::with_capacity(d.rgba.len());
        for px in d.rgba.chunks_exact(4) {
            bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let surface = gtk4::cairo::ImageSurface::create_for_data(
            bgra,
            gtk4::cairo::Format::ARgb32,
            d.width as i32,
            d.height as i32,
            (d.width * 4) as i32,
        )
        .expect("surface");
        *self.surface.borrow_mut() = Some(surface);
        self.img_w.set(d.width as f64);
        self.img_h.set(d.height as f64);
        self.zoom.set(1.0);
        self.off_x.set(0.0);
        self.off_y.set(0.0);
        self.fit_to_window();
    }

    /// Fit-window zoom for the current area size.
    pub fn fit_to_window(&self) {
        let w = self.area.width() as f64;
        let h = self.area.height() as f64;
        let (tw, th) = (self.img_w.get(), self.img_h.get());
        if tw > 0.0 && th > 0.0 {
            self.zoom.set((w / tw).min(h / th).max(0.01));
        }
        self.off_x.set(0.0);
        self.off_y.set(0.0);
        self.fit.set(FitMode::FitWindow);
        self.area.queue_draw();
    }

    /// Zoom to 100%.
    pub fn actual_size(&self) {
        self.zoom.set(1.0);
        self.fit.set(FitMode::ActualSize);
        self.area.queue_draw();
    }

    /// Step zoom via keyboard (+/-).
    pub fn zoom_step(&self, factor: f64) {
        self.zoom.set((self.zoom.get() * factor).clamp(0.01, 256.0));
        self.fit.set(FitMode::ActualSize);
        self.area.queue_draw();
    }
}
