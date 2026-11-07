// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: canvas, drag, select, thumbs
// Copyright (C) 2026 LieraOS Project
//
// Visual monitor arrangement with drag-and-drop, snapping, click
// selection and live thumbnails. Controls live in display_controls.

use gtk4::gdk;
use gtk4::prelude::*;
use ragnar_shared::display::Head;
use std::cell::RefCell;
use std::rc::Rc;

use super::dbus_client;

/// Scale factor: monitor pixels to canvas pixels.
pub(crate) const SCALE: f64 = 0.10;
/// Canvas dimensions in pixels.
const CANVAS_W: i32 = 600;
const CANVAS_H: i32 = 300;
/// Snap distance in canvas pixels.
const SNAP_DIST: f64 = 12.0;
/// Directory for temporary monitor screenshots.
const THUMB_DIR: &str = "/tmp/ragnar-settings-thumbs";

/// Shared UI state between canvas and controls.
/// Both sides mutate via RefCell; `refresh` is the controls-side
/// callback that syncs dropdown/switches to the current selection.
pub(crate) struct DisplayUi {
    pub heads: Rc<RefCell<Vec<Head>>>,
    pub positions: Rc<RefCell<Vec<(String, f64, f64)>>>,
    pub selected: Rc<RefCell<Option<usize>>>,
    pub frames: Rc<RefCell<Vec<gtk4::Frame>>>,
    pub primary_marks: Rc<RefCell<Vec<gtk4::Label>>>,
    pub refresh: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
}

/// Build the Display settings page.
pub fn create_page() -> gtk4::Widget {
    // App-wide CSS: selection highlight + primary badge styling.
    let css = gtk4::CssProvider::new();
    css.load_from_data(
        "frame.selected { border: 2px solid #3584e4; background-color: alpha(#3584e4, 0.15); }\
         .primary-badge { color: #ffffff; font-weight: bold; }",
    );
    if let Some(disp) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &disp,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 24);
    container.set_margin_start(24);
    container.set_margin_end(24);
    container.set_margin_top(24);
    container.set_margin_bottom(24);

    container.append(&create_header());

    let heads = dbus_client::fetch_outputs();

    let ui = Rc::new(DisplayUi {
        heads: Rc::new(RefCell::new(heads.to_vec())),
        positions: Rc::new(RefCell::new(
            heads
                .iter()
                .map(|h| {
                    (
                        h.name.clone(),
                        h.position.0 as f64 * SCALE,
                        h.position.1 as f64 * SCALE,
                    )
                })
                .collect(),
        )),
        selected: Rc::new(RefCell::new(if heads.is_empty() { None } else { Some(0) })),
        frames: Rc::new(RefCell::new(Vec::new())),
        primary_marks: Rc::new(RefCell::new(Vec::new())),
        refresh: Rc::new(RefCell::new(None)),
    });

    container.append(&create_canvas(&ui));
    container.append(&super::controls::create_controls(&ui));
    container.append(&super::gpu::create_gpu_section());

    // Initial highlight of the pre-selected monitor.
    if let Ok(f) = ui.frames.try_borrow() {
        if let (Some(idx), Some(fr)) = (*ui.selected.borrow(), f.first()) {
            if f.len() > idx {
                f[idx].add_css_class("selected");
            }
            let _ = fr; // silence unused
        }
    }

    container.upcast()
}

/// Re-highlight all monitor frames so exactly `sel` carries .selected.
pub(crate) fn restyle_frames(ui: &DisplayUi) {
    let frames = ui.frames.borrow();
    let sel = *ui.selected.borrow();
    for (i, fr) in frames.iter().enumerate() {
        if Some(i) == sel {
            fr.add_css_class("selected");
        } else {
            fr.remove_css_class("selected");
        }
    }
    // Refresh controls widgets if registered.
    if let Some(f) = ui.refresh.borrow().as_ref() {
        f();
    }
}

/// Section header with title and description.
fn create_header() -> gtk4::Box {
    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    box_.append(
        &gtk4::Label::builder()
            .label("Display")
            .xalign(0.0)
            .css_classes(["title-1"])
            .build(),
    );
    box_.append(
        &gtk4::Label::builder()
            .label("Drag monitors to arrange them. Click to select.")
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    box_
}

/// Canvas area where monitors are shown as draggable thumbnails.
fn create_canvas(ui: &Rc<DisplayUi>) -> gtk4::Frame {
    let frame = gtk4::Frame::new(Some("Monitor Arrangement"));
    let fixed = gtk4::Fixed::new();
    fixed.set_size_request(CANVAS_W, CANVAS_H);
    fixed.set_hexpand(false);
    fixed.set_vexpand(false);

    let heads = ui.heads.borrow();
    if heads.is_empty() {
        let label = gtk4::Label::new(Some(
            "No monitors detected — is ragnar-display-svc running?",
        ));
        label.set_margin_top(120);
        label.set_margin_start(120);
        fixed.put(&label, 0.0, 0.0);
    } else {
        let rects: Vec<MonitorRect> = heads
            .iter()
            .map(|h| MonitorRect::from_head(h, SCALE))
            .collect();

        for (i, head) in heads.iter().enumerate() {
            let rect = create_monitor_rect(ui, head, i);
            let pos = rects[i].pos;
            setup_drag(
                &rect,
                &fixed,
                pos.0,
                pos.1,
                &rects,
                i,
                head.name.clone(),
                ui,
            );
            setup_select(&rect, i, ui);
            fixed.put(&rect, pos.0, pos.1);
            ui.frames.borrow_mut().push(rect);
        }
    }
    frame.set_child(Some(&fixed));
    frame
}

/// A monitor's position and size on the canvas.
#[derive(Clone, Copy)]
struct MonitorRect {
    pos: (f64, f64),
    w: f64,
    h: f64,
}

impl MonitorRect {
    fn from_head(head: &Head, scale: f64) -> Self {
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
fn create_monitor_rect(ui: &Rc<DisplayUi>, head: &Head, _index: usize) -> gtk4::Frame {
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
fn setup_select(rect: &gtk4::Frame, index: usize, ui: &Rc<DisplayUi>) {
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

/// Set up drag with snapping, boundary clamping, and position tracking.
fn setup_drag(
    rect: &gtk4::Frame,
    fixed: &gtk4::Fixed,
    start_x: f64,
    start_y: f64,
    all_rects: &[MonitorRect],
    self_index: usize,
    monitor_name: String,
    ui: &Rc<DisplayUi>,
) {
    use std::cell::Cell;

    let gesture = gtk4::GestureDrag::builder()
        .button(gdk::BUTTON_PRIMARY)
        .propagation_phase(gtk4::PropagationPhase::Capture)
        .build();

    let fixed_clone = fixed.clone();
    let rect_clone = rect.clone();
    let rects = all_rects.to_vec();
    let pos_state = ui.positions.clone();
    let name = monitor_name;
    let origin_x = Cell::new(start_x);
    let origin_y = Cell::new(start_y);

    gesture.connect_drag_begin(move |_g, _sx, _sy| {});

    gesture.connect_drag_update(move |_g, offset_x, offset_y| {
        let mut new_x = origin_x.get() + offset_x;
        let mut new_y = origin_y.get() + offset_y;
        let my_w = rects[self_index].w;
        let my_h = rects[self_index].h;

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
            if j == self_index {
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

    rect.add_controller(gesture);
}
