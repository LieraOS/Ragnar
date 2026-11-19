// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: monitor rect widgets
// Copyright (C) 2026 LieraOS Project
//
// Per-monitor canvas widgets: the draggable frame with live
// thumbnail, name overlay and primary badge, plus the click-to-
// select and drag-and-drop (snap/clamp) controllers. The canvas
// container and page assembly live in canvas.rs.

use gtk4::gdk;
use gtk4::prelude::*;
use ragnar_shared::display::Head;
use std::cell::Cell;
use std::rc::Rc;

use super::canvas::{restyle_frames, DisplayUi, CANVAS_H, CANVAS_W, SCALE};

/// Snap distance in canvas pixels.
const SNAP_DIST: f64 = 12.0;
/// Directory for temporary monitor screenshots.
const THUMB_DIR: &str = "/tmp/ragnar-settings-thumbs";

/// A monitor's position and size on the canvas.
#[derive(Clone, Copy)]
pub(crate) struct MonitorRect {
    pub pos: (f64, f64),
    pub w: f64,
    pub h: f64,
}

impl MonitorRect {
    pub fn from_head(head: &Head, scale: f64) -> Self {
        let w = (head.current_mode.width as f64 * scale).max(70.0);
        let h = (head.current_mode.height as f64 * scale).max(45.0);
        MonitorRect {
            pos: (
                head.position.0 as f64 * scale,
                head.position.1 as f64 * scale,
            ),
            w,
            h,
        }
    }
    fn right(&self) -> f64 {
        self.pos.0 + self.w
    }
    fn bottom(&self) -> f64 {
        self.pos.1 + self.h
    }
}

/// Create a visual rectangle representing a single monitor.
/// Live thumbnail + name overlay + primary badge.
pub(crate) fn create_monitor_rect(ui: &Rc<DisplayUi>, head: &Head, _index: usize) -> gtk4::Frame {
    let w = ((head.current_mode.width as f64 * SCALE) as i32).max(70);
    let h = ((head.current_mode.height as f64 * SCALE) as i32).max(45);

    let frame = gtk4::Frame::builder()
        .width_request(w)
        .height_request(h)
        .css_classes(["card"])
        .build();

    let overlay = gtk4::Overlay::new();
    let image = gtk4::Image::new();
    image.set_pixel_size(w);
    overlay.set_child(Some(&image));

    let name = gtk4::Label::builder()
        .label(&head.name)
        .css_classes(["caption", "heading"])
        .halign(gtk4::Align::Start)
        .valign(gtk4::Align::End)
        .margin_start(4)
        .margin_bottom(2)
        .build();
    overlay.add_overlay(&name);

    // Primary badge ("P") at top-left corner.
    let badge = gtk4::Label::builder()
        .label("P")
        .css_classes(["primary-badge"])
        .halign(gtk4::Align::Start)
        .valign(gtk4::Align::Start)
        .margin_start(4)
        .margin_top(2)
        .visible(head.primary)
        .build();
    overlay.add_overlay(&badge);
    ui.primary_marks.borrow_mut().push(badge);

    refresh_thumb(&image, &head.name, w, h);
    // Periodic thumbnail refresh — but only while this widget still
    // exists (page alive) and is mapped (page visible). grim runs on
    // a worker thread so the UI never blocks on the capture; only the
    // cheap pixbuf load happens on the main thread.
    let image_w = image.downgrade();
    let name_c = head.name.clone();
    gtk4::glib::timeout_add_seconds_local(2, move || {
        let Some(image) = image_w.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };
        if !image.is_mapped() {
            return gtk4::glib::ControlFlow::Continue;
        }
        let name = name_c.clone();
        let image_c = image.clone();
        gtk4::glib::spawn_future_local(async move {
            if let Some(path) = gtk4::gio::spawn_blocking(move || capture_monitor_thumb(&name))
                .await
                .ok()
                .flatten()
            {
                apply_thumb(&image_c, &path, w, h);
            }
        });
        gtk4::glib::ControlFlow::Continue
    });

    frame.set_child(Some(&overlay));
    frame
}

/// Re-capture a monitor screenshot and update the image widget.
fn refresh_thumb(image: &gtk4::Image, output: &str, w: i32, h: i32) {
    if let Some(path) = capture_monitor_thumb(output) {
        apply_thumb(image, &path, w, h);
    }
}

/// Load a captured PNG into the image widget, scaled to the rect.
fn apply_thumb(image: &gtk4::Image, path: &str, w: i32, h: i32) {
    if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_file(path) {
        if let Some(scaled) =
            pixbuf.scale_simple(w.max(1), h.max(1), gdk_pixbuf::InterpType::Bilinear)
        {
            image.set_from_pixbuf(Some(&scaled));
        }
    }
}

/// Capture a screenshot of a specific output using grim.
fn capture_monitor_thumb(output: &str) -> Option<String> {
    use std::fs;
    use std::process::Command;
    let _ = fs::create_dir_all(THUMB_DIR);
    let path = format!("{}/{}.png", THUMB_DIR, output);
    match Command::new("grim").args(["-o", output, &path]).output() {
        Ok(out) if out.status.success() => Some(path),
        _ => None,
    }
}

/// Set up click-to-select: clicking the rect selects this monitor
/// and refreshes controls via the shared refresh callback.
pub(crate) fn setup_select(rect: &gtk4::Frame, index: usize, ui: &Rc<DisplayUi>) {
    let click = gtk4::GestureClick::builder()
        .button(gdk::BUTTON_PRIMARY)
        .build();
    let ui_c = ui.clone();
    click.connect_pressed(move |_g, _n, _x, _y| {
        *ui_c.selected.borrow_mut() = Some(index);
        restyle_frames(&ui_c);
    });
    rect.add_controller(click);
}

/// Everything drag setup needs — bundled so the call site stays
/// readable instead of an eight-parameter invocation.
pub(crate) struct DragCtx<'a> {
    pub rect: &'a gtk4::Frame,
    pub fixed: &'a gtk4::Fixed,
    pub start: (f64, f64),
    pub all_rects: &'a [MonitorRect],
    pub self_index: usize,
    pub monitor_name: String,
    pub ui: &'a Rc<DisplayUi>,
}

/// Set up drag with snapping, boundary clamping, and position tracking.
pub(crate) fn setup_drag(ctx: DragCtx) {
    let gesture = gtk4::GestureDrag::builder()
        .button(gdk::BUTTON_PRIMARY)
        .propagation_phase(gtk4::PropagationPhase::Capture)
        .build();

    let fixed_clone = ctx.fixed.clone();
    let rect_clone = ctx.rect.clone();
    let rects = ctx.all_rects.to_vec();
    let pos_state = ctx.ui.positions.clone();
    let name = ctx.monitor_name;
    let origin_x = Cell::new(ctx.start.0);
    let origin_y = Cell::new(ctx.start.1);

    gesture.connect_drag_begin(move |_g, _sx, _sy| {});

    gesture.connect_drag_update(move |_g, offset_x, offset_y| {
        let mut new_x = origin_x.get() + offset_x;
        let mut new_y = origin_y.get() + offset_y;
        let my_w = rects[ctx.self_index].w;
        let my_h = rects[ctx.self_index].h;

        new_x = new_x.max(0.0).min((CANVAS_W as f64) - my_w);
        new_y = new_y.max(0.0).min((CANVAS_H as f64) - my_h);

        if new_x < SNAP_DIST {
            new_x = 0.0;
        }
        if new_y < SNAP_DIST {
            new_y = 0.0;
        }
        if (CANVAS_W as f64) - my_w - new_x < SNAP_DIST {
            new_x = (CANVAS_W as f64) - my_w;
        }
        if (CANVAS_H as f64) - my_h - new_y < SNAP_DIST {
            new_y = (CANVAS_H as f64) - my_h;
        }

        for (j, other) in rects.iter().enumerate() {
            if j == ctx.self_index {
                continue;
            }
            if (new_x - other.right()).abs() < SNAP_DIST
                && new_y < other.bottom()
                && new_y + my_h > other.pos.1
            {
                new_x = other.right();
            }
            if (new_x + my_w - other.pos.0).abs() < SNAP_DIST
                && new_y < other.bottom()
                && new_y + my_h > other.pos.1
            {
                new_x = other.pos.0 - my_w;
            }
            if (new_y - other.bottom()).abs() < SNAP_DIST
                && new_x < other.right()
                && new_x + my_w > other.pos.0
            {
                new_y = other.bottom();
            }
            if (new_y + my_h - other.pos.1).abs() < SNAP_DIST
                && new_x < other.right()
                && new_x + my_w > other.pos.0
            {
                new_y = other.pos.1 - my_h;
            }
            if (new_y - other.pos.1).abs() < SNAP_DIST {
                new_y = other.pos.1;
            }
            if (new_y + my_h - other.bottom()).abs() < SNAP_DIST {
                new_y = other.bottom() - my_h;
            }
            if (new_x - other.pos.0).abs() < SNAP_DIST {
                new_x = other.pos.0;
            }
            if (new_x + my_w - other.right()).abs() < SNAP_DIST {
                new_x = other.right() - my_w;
            }
        }

        origin_x.set(new_x);
        origin_y.set(new_y);

        if let Ok(mut pos) = pos_state.try_borrow_mut() {
            for entry in pos.iter_mut() {
                if entry.0 == name {
                    entry.1 = new_x;
                    entry.2 = new_y;
                }
            }
        }
        fixed_clone.move_(&rect_clone, new_x, new_y);
    });

    ctx.rect.add_controller(gesture);
}
