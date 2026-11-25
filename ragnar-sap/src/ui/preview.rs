// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — interactive scan preview widget
//
// (the drawing area): wheel zoom (factor
// 1.15), drag-pan, and a redact mode where left-drag draws a dashed
// rubber band that becomes an opaque black redaction box on release.
// Redaction rects live in *image* coordinates, so they apply at full
// resolution regardless of zoom — exactly like the design.
// Rendering uses a DrawingArea with a cairo surface cached per page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::glib;

use crate::image::pnm::Rgb8Image;
use crate::image::redact::Rect;

/// Scroll zoom factor — kZoomFactor.
const ZOOM: f64 = 1.15;
/// Ignore accidental sub-pixel drags.
const MIN_RECT: f64 = 2.0;

/// Mutable view + redaction state behind one Rc for the controllers.
struct Inner {
 image: RefCell<Option<Rgb8Image>>,
 /// cairo surface of the image (Rgb24, cached per page).
 surface: RefCell<Option<gtk4::cairo::ImageSurface>>,
 scale: Cell<f64>,
 offset: Cell<(f64, f64)>,
 redact_mode: Cell<bool>,
 redactions: RefCell<Vec<Rect>>,
 drag_start: Cell<(f64, f64)>,
 drag_now: Cell<(f64, f64)>,
 dragging_redact: Cell<bool>,
 /// Fired whenever the redaction list changes (undo/clear/add) —
 /// the window uses it to enable/disable Undo and Clear.
 on_change: RefCell<Box<dyn Fn(usize)>>,
}

/// Redaction-capable preview widget.
#[derive(Clone)]
pub struct ScanPreview {
 pub area: gtk4::DrawingArea,
 inner: Rc<Inner>,
}

impl ScanPreview {
 /// Build the widget with all gestures wired.
 pub fn new() -> Self {
 let inner = Rc::new(Inner {
 image: RefCell::new(None),
 surface: RefCell::new(None),
 scale: Cell::new(1.0),
 offset: Cell::new((0.0, 0.0)),
 redact_mode: Cell::new(false),
 redactions: RefCell::new(Vec::new()),
 drag_start: Cell::new((0.0, 0.0)),
 drag_now: Cell::new((0.0, 0.0)),
 dragging_redact: Cell::new(false),
 on_change: RefCell::new(Box::new(|_| {})),
 });

 let area = gtk4::DrawingArea::new();
 area.set_hexpand(true);
 area.set_vexpand(true);
 area.set_focusable(true);

 // ---- draw: cairo surface + rects ---------------------------
 area.set_draw_func({
 let inner = inner.clone();
 move |_, ctx, _w, _h| {
 inner.draw(ctx);
 }
 });

 // ---- wheel zoom (1.15x like the drawing area) ----------
 let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL,
 );
 scroll.connect_scroll({
 let inner = inner.clone();
 let area = area.clone();
 move |_, _dx, dy| {
 let factor = if dy < 0.0 { ZOOM } else { 1.0 / ZOOM };
 inner.scale.set((inner.scale.get() * factor).clamp(0.05, 40.0));
 area.queue_draw();
 glib::Propagation::Stop
 }
 });
 area.add_controller(scroll);

 // ---- drag: pan (default) / rubber band (redact mode) --------
 let drag = gtk4::GestureDrag::new();
 drag.set_button(0); // any button — branch on mode/button below
 drag.connect_drag_begin({
 let inner = inner.clone();
 let area = area.clone();
 move |gesture, x, y| {
 if inner.redact_mode.get() && gesture.current_button() == 1 {
 // Left button in redact mode: rubber band starts in
 // image coordinates.
 let p = inner.view_to_image(x, y);
 inner.drag_start.set(p);
 inner.drag_now.set(p);
 inner.dragging_redact.set(true);
 } else {
 // Panning (any button): store start for offset math.
 inner.drag_start.set((x, y));
 inner.dragging_redact.set(false);
 }
 area.queue_draw();
 }
 });
 drag.connect_drag_update({
 let inner = inner.clone();
 let area = area.clone();
 move |_, dx, dy| {
 if inner.dragging_redact.get() {
 // Band end = start re-projected to view space plus
 // the gesture offset, back in image coordinates so
 // the box survives zoom changes.
 let (sx, sy) = inner.drag_start.get();
 let (vx, vy) = inner.image_to_view(sx, sy);
 inner.drag_now.set(inner.view_to_image(vx + dx, vy + dy));
 area.queue_draw();
 }
 }
 });
 drag.connect_drag_end({
 let inner = inner.clone();
 let area = area.clone();
 move |_, dx, dy| {
 if inner.dragging_redact.get() {
 let (sx, sy) = inner.drag_start.get();
 let (vx, vy) = inner.image_to_view(sx, sy);
 let (ex, ey) = inner.view_to_image(vx + dx, vy + dy);
 let rect = Rect::from_points(sx, sy, ex, ey);
 if rect.width as f64 > MIN_RECT && rect.height as f64 > MIN_RECT {
 if let Some(img) = inner.image.borrow().as_ref() {
 let r = rect.clipped(img.width as i32, img.height as i32);
 inner.redactions.borrow_mut().push(r);
 (inner.on_change.borrow())(inner.redactions.borrow().len());
 }
 }
 inner.dragging_redact.set(false);
 } else {
 // Pan: apply the view-space delta to the offset.
 let (ox, oy) = inner.offset.get();
 inner.offset.set((ox + dx, oy + dy));
 }
 area.queue_draw();
 }
 });
 area.add_controller(drag);

 Self { area, inner }
 }

 /// Replace the shown page and fit it into the view.
 pub fn set_image(&self, img: &Rgb8Image, keep_view: bool) {
 *self.inner.surface.borrow_mut() = Some(to_surface(img));
 *self.inner.image.borrow_mut() = Some(img.clone());
 if !keep_view {
 self.fit();
 }
 self.area.queue_draw();
 }

 /// Fit the image into the current widget size.
 fn fit(&self) {
 let img = self.inner.image.borrow();
 let Some(img) = img.as_ref() else { return };
 let (w, h) = (self.area.width() as f64, self.area.height() as f64);
 if w <= 1.0 || h <= 1.0 {
 self.inner.scale.set(1.0);
 self.inner.offset.set((0.0, 0.0));
 return;
 }
 let scale = (w / img.width as f64).min(h / img.height as f64);
 self.inner.scale.set(scale);
 let ox = (w - img.width as f64 * scale) / 2.0;
 let oy = (h - img.height as f64 * scale) / 2.0;
 self.inner.offset.set((ox, oy));
 }

 /// Enable/disable the redaction mode.
 pub fn set_redact_mode(&self, enabled: bool) {
 self.inner.redact_mode.set(enabled);
 self.area.set_cursor_from_name(Some(if enabled { "crosshair" } else { "grab" }));
 }

 /// Remove the last redaction box.
 pub fn undo_redaction(&self) {
 self.inner.redactions.borrow_mut().pop();
 (self.inner.on_change.borrow())(self.inner.redactions.borrow().len());
 self.area.queue_draw();
 }

 /// Remove every redaction box.
 pub fn clear_redactions(&self) {
 self.inner.redactions.borrow_mut().clear();
 (self.inner.on_change.borrow())(0);
 self.area.queue_draw();
 }

 /// Current redaction rectangles in image coordinates.
 pub fn redaction_rects(&self) -> Vec<Rect> {
 self.inner.redactions.borrow().clone()
 }

 /// Replace the redaction list (page-switch restore).
 pub fn set_redactions(&self, rects: Vec<Rect>) {
 *self.inner.redactions.borrow_mut() = rects;
 (self.inner.on_change.borrow())(self.inner.redactions.borrow().len());
 self.area.queue_draw();
 }

 /// Register the redaction-count callback.
 pub fn on_redactions_changed(&self, cb: impl Fn(usize) + 'static) {
 *self.inner.on_change.borrow_mut() = Box::new(cb);
 }
}

/// RGB8 → cairo Rgb24 surface (little-endian BGRX, cached once).
fn to_surface(img: &Rgb8Image) -> gtk4::cairo::ImageSurface {
 let (w, h) = (img.width as i32, img.height as i32);
 let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::Rgb24, w, h)
.expect("image surface");
 {
 let stride = surface.stride() as usize;
 let mut data = surface.data().expect("surface data");
 for y in 0..img.height {
 for x in 0..img.width {
 let si = y * stride + x * 4;
 let di = (y * img.width + x) * 3;
 data[si] = img.data[di + 2]; // B
 data[si + 1] = img.data[di + 1]; // G
 data[si + 2] = img.data[di]; // R
 data[si + 3] = 0xFF; // X (unused)
 }
 }
 }
 surface.mark_dirty();
 surface
}

impl Inner {
 /// View → image coordinates (inverse of draw transform).
 fn view_to_image(&self, vx: f64, vy: f64) -> (f64, f64) {
 let (ox, oy) = self.offset.get();
 ((vx - ox) / self.scale.get(), (vy - oy) / self.scale.get())
 }

 /// Image → view coordinates.
 fn image_to_view(&self, ix: f64, iy: f64) -> (f64, f64) {
 let (ox, oy) = self.offset.get();
 (ix * self.scale.get() + ox, iy * self.scale.get() + oy)
 }

 /// Paint image, committed redactions and the live rubber band.
 fn draw(&self, ctx: &gtk4::cairo::Context) {
 // Checker-free dark canvas behind the page.
 ctx.set_source_rgb(0.18, 0.18, 0.18);
 let _ = ctx.paint();

 let Some(surface) = self.surface.borrow().clone() else { return };
 let scale = self.scale.get();
 let (ox, oy) = self.offset.get();

 ctx.save().expect("save");
 ctx.translate(ox, oy);
 ctx.scale(scale, scale);
 ctx.set_source_surface(&surface, 0.0, 0.0).expect("source");
 ctx.paint().expect("paint");
 ctx.restore().expect("restore");

 // Committed redactions: opaque black, thin red outline.
 for r in self.redactions.borrow().iter() {
 let (x, y) = self.image_to_view(r.x as f64, r.y as f64);
 let w = r.width as f64 * scale;
 let h = r.height as f64 * scale;
 ctx.set_source_rgb(0.0, 0.0, 0.0);
 ctx.rectangle(x, y, w, h);
 let _ = ctx.fill_preserve();
 ctx.set_source_rgb(1.0, 0.0, 0.0);
 ctx.set_line_width(1.5);
 let _ = ctx.stroke();
 }

 // Live rubber band while dragging in redact mode (dashed).
 if self.dragging_redact.get() {
 let (sx, sy) = self.drag_start.get();
 let (ex, ey) = self.drag_now.get();
 let (x1, y1) = self.image_to_view(sx.min(ex), sy.min(ey));
 let w = (ex - sx).abs() * scale;
 let h = (ey - sy).abs() * scale;
 ctx.set_dash(&[4.0, 4.0], 0.0);
 ctx.set_source_rgb(1.0, 0.0, 0.0);
 ctx.rectangle(x1, y1, w, h);
 ctx.set_line_width(1.0);
 let _ = ctx.stroke();
 ctx.set_dash(&[], 0.0);
 }
 }
}
