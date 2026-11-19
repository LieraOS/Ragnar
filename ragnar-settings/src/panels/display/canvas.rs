// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: page assembly & canvas
// Copyright (C) 2026 LieraOS Project
//
// Visual monitor arrangement page: header, the Fixed-canvas that
// hosts the draggable monitor rects, and the shared UI state.
// Rect creation, drag/select controllers and thumbnails live in
// monitor_rect.rs; controls live in display_controls.

use gettextrs::gettext;
use gtk4::prelude::*;
use ragnar_shared::display::Head;
use std::cell::RefCell;
use std::rc::Rc;

use super::dbus_client;
use super::monitor_rect::{create_monitor_rect, setup_drag, setup_select, DragCtx, MonitorRect};

/// Scale factor: monitor pixels to canvas pixels.
pub(crate) const SCALE: f64 = 0.10;
/// Canvas dimensions in pixels.
pub(crate) const CANVAS_W: i32 = 600;
pub(crate) const CANVAS_H: i32 = 300;

/// Controls-side callback that syncs dropdowns/switches to the
/// current selection — stored on the shared state so restyle and
/// drag updates can trigger it.
pub(crate) type RefreshFn = Rc<dyn Fn()>;

/// Shared UI state between canvas and controls.
/// Both sides mutate via RefCell; `refresh` is the controls-side
/// callback that syncs dropdown/switches to the current selection.
pub(crate) struct DisplayUi {
    pub heads: Rc<RefCell<Vec<Head>>>,
    pub positions: Rc<RefCell<Vec<(String, f64, f64)>>>,
    pub selected: Rc<RefCell<Option<usize>>>,
    pub frames: Rc<RefCell<Vec<gtk4::Frame>>>,
    pub primary_marks: Rc<RefCell<Vec<gtk4::Label>>>,
    pub refresh: Rc<RefCell<Option<RefreshFn>>>,
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
    // GPU power/fan/clock controls are advanced territory — hidden
    // in the normal view (the tile itself stays visible).
    if crate::advanced::enabled() {
        container.append(&super::gpu::create_gpu_section());
    }

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
            .label(gettext("Display"))
            .xalign(0.0)
            .css_classes(["title-1"])
            .build(),
    );
    box_.append(
        &gtk4::Label::builder()
            .label(gettext("Drag monitors to arrange them. Click to select."))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    box_
}

/// Canvas area where monitors are shown as draggable thumbnails.
fn create_canvas(ui: &Rc<DisplayUi>) -> gtk4::Frame {
    let frame = gtk4::Frame::new(Some(&gettext("Monitor Arrangement")));
    let fixed = gtk4::Fixed::new();
    fixed.set_size_request(CANVAS_W, CANVAS_H);
    fixed.set_hexpand(false);
    fixed.set_vexpand(false);

    let heads = ui.heads.borrow();
    if heads.is_empty() {
        let label = gtk4::Label::new(Some(&gettext(
            "No monitors detected — is ragnar-display-svc running?",
        )));
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
            setup_drag(DragCtx {
                rect: &rect,
                fixed: &fixed,
                start: pos,
                all_rects: &rects,
                self_index: i,
                monitor_name: head.name.clone(),
                ui,
            });
            setup_select(&rect, i, ui);
            fixed.put(&rect, pos.0, pos.1);
            ui.frames.borrow_mut().push(rect);
        }
    }
    frame.set_child(Some(&fixed));
    frame
}
