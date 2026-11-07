// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: controls + D-Bus wiring
// Copyright (C) 2026 LieraOS Project
//
// Monitor selector, resolution/refresh dropdowns, primary and
// enabled toggles, Apply button. All widgets sync with the
// monitor selected in the canvas via DisplayUi state.

use gtk4::prelude::*;
use std::rc::Rc;

use super::canvas::{restyle_frames, DisplayUi, SCALE};
use super::dbus_client;

/// Create the controls section bound to the shared display state.
pub fn create_controls(ui: &Rc<DisplayUi>) -> gtk4::Box {
    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

    if ui.heads.borrow().is_empty() {
        box_.append(&gtk4::Label::new(Some(
            "Start ragnar-display-svc to configure displays.",
        )));
        return box_;
    }

    // -- Monitor selector ------------------------------------------------
    let monitor_names: Vec<String> = ui
        .heads
        .borrow()
        .iter()
        .map(|h| format!("{} ({})", h.name, h.model))
        .collect();
    let name_refs: Vec<&str> = monitor_names.iter().map(|s| s.as_str()).collect();
    let monitor_dropdown = gtk4::DropDown::from_strings(&name_refs);
    monitor_dropdown.set_selected(ui.selected.borrow().unwrap_or(0) as u32);
    box_.append(&dropdown_row("Monitor", &monitor_dropdown));

    // Selecting from the dropdown also selects in the canvas.
    {
        let ui_c = ui.clone();
        monitor_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if *ui_c.selected.borrow() != Some(idx) {
                *ui_c.selected.borrow_mut() = Some(idx);
                restyle_frames(&ui_c);
            }
        });
    }

    // -- Resolution / refresh / scale dropdowns --------------------------
    let mode = super::mode_controls::create(ui);
    box_.append(&mode.widget);

    // -- Primary toggle --------------------------------------------------
    let (primary_row, primary_switch) = toggle_row("Primary Monitor");
    box_.append(&primary_row);
    let primary_hid = {
        let ui_c = ui.clone();
        primary_switch.connect_state_set(move |_, state| {
            if let Some(idx) = *ui_c.selected.borrow() {
                let name = ui_c.heads.borrow()[idx].name.clone();
                if state {
                    eprintln!("ragnar-settings: set_primary {}", name);
                    let r1 = dbus_client::send_primary(&name);
                    eprintln!("  set_primary: {} {}", r1.success, r1.message);
                    let r2 = dbus_client::send_apply();
                    eprintln!("  apply: {} {}", r2.success, r2.message);
                    // Update local state + badges.
                    for (i, h) in ui_c.heads.borrow_mut().iter_mut().enumerate() {
                        h.primary = i == idx;
                    }
                    refresh_primary_marks(&ui_c);
                }
            }
            gtk4::glib::Propagation::Proceed
        })
    };

    // -- Enabled toggle --------------------------------------------------
    let (enable_row, enable_switch) = toggle_row("Enabled");
    box_.append(&enable_row);
    let enable_hid = {
        let ui_c = ui.clone();
        enable_switch.connect_state_set(move |_, state| {
            if let Some(idx) = *ui_c.selected.borrow() {
                let name = ui_c.heads.borrow()[idx].name.clone();
                eprintln!("ragnar-settings: enable_output {} = {}", name, state);
                let r1 = dbus_client::send_enable(&name, state);
                eprintln!("  enable_output: {} {}", r1.success, r1.message);
                let r2 = dbus_client::send_apply();
                eprintln!("  apply: {} {}", r2.success, r2.message);
                ui_c.heads.borrow_mut()[idx].enabled = state;
            }
            gtk4::glib::Propagation::Proceed
        })
    };

    // -- Apply button ----------------------------------------------------
    let apply = gtk4::Button::with_label("Apply");
    apply.set_halign(gtk4::Align::End);
    apply.add_css_class("suggested-action");
    {
        let ui_c = ui.clone();
        apply.connect_clicked(move |_| {
            let pos = ui_c.positions.borrow();
            for (name, cx, cy) in pos.iter() {
                let rx = (cx / SCALE) as i32;
                let ry = (cy / SCALE) as i32;
                eprintln!("ragnar-settings: applying {} -> ({}, {})", name, rx, ry);
                let r = dbus_client::send_position(name, rx, ry);
                eprintln!("  set_position: {} {}", r.success, r.message);
            }
            let r = dbus_client::send_apply();
            eprintln!("ragnar-settings: apply {} {}", r.success, r.message);
        });
    }
    box_.append(&apply);

    // -- Refresh callback: sync widgets to the selected head --------------
    {
        let ui_c = ui.clone();
        let dd = monitor_dropdown.clone();
        let primary = primary_switch.clone();
        let enable = enable_switch.clone();
        let mode_populate = mode.populate.clone();
        let p_hid = primary_hid;
        let e_hid = enable_hid;
        *ui.refresh.borrow_mut() = Some(Rc::new(move || {
            let sel = match *ui_c.selected.borrow() {
                Some(i) => i,
                None => return,
            };
            let head = {
                let heads = ui_c.heads.borrow();
                match heads.get(sel) {
                    Some(h) => h.clone(),
                    None => return,
                }
            };

            dd.set_selected(sel as u32);
            // Block user-handler while syncing programmatic state —
            // prevents re-entrant borrow + spurious D-Bus calls.
            primary.block_signal(&p_hid);
            enable.block_signal(&e_hid);
            primary.set_active(head.primary);
            enable.set_active(head.enabled);
            primary.unblock_signal(&p_hid);
            enable.unblock_signal(&e_hid);

            mode_populate(&head);
        }));
    }

    // Initial sync.
    restyle_frames(ui);

    box_
}

/// Show/hide the "P" badge on each monitor according to primary flag.
fn refresh_primary_marks(ui: &Rc<DisplayUi>) {
    let heads = ui.heads.borrow();
    let marks = ui.primary_marks.borrow();
    for (i, mark) in marks.iter().enumerate() {
        mark.set_visible(heads.get(i).map(|h| h.primary).unwrap_or(false));
    }
}

/// Row with a label and an existing dropdown widget.
fn dropdown_row(label: &str, dd: &gtk4::DropDown) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    row.set_hexpand(true);
    row.append(
        &gtk4::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    row.append(dd);
    row
}

/// Row with a label on the left and a read-only value label on the right.
/// Toggle row: label on the left, switch on the right.
fn toggle_row(label: &str) -> (gtk4::Box, gtk4::Switch) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    row.set_hexpand(true);
    row.append(
        &gtk4::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    let switch = gtk4::Switch::new();
    row.append(&switch);
    (row, switch)
}
