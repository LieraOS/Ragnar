// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: mode/scale/rotation/sync controls
// Copyright (C) 2026 LieraOS Project
//
// Resolution, refresh-rate, scale, rotation and adaptive-sync controls
// for the selected monitor. Changes are queued on the daemon and
// applied via ApplyConfig (wlr-randr). A mode change opens the 15 s
// revert dialog so a bad mode can never leave the user blind.
// Programmatic repopulation on monitor switch blocks all handlers to
// avoid feedback loops and spurious D-Bus calls.

use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::canvas::DisplayUi;
use super::dbus_client;
use ragnar_shared::display::{Head, Mode};

/// Widget + repopulation entry point for the mode controls.
pub struct ModeControls {
    pub widget: gtk4::Box,
    /// Rebuild all controls for the given head (handlers blocked).
    pub populate: Rc<dyn Fn(&Head)>,
}

/// Scale factors offered in the dropdown.
const SCALES: &[f64] = &[0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
const SCALE_LABELS: &[&str] = &[
    "75 %", "100 %", "125 %", "150 %", "175 %", "200 %", "250 %", "300 %",
];
/// Rotation options mapped to degrees.
const ROTATIONS: &[i32] = &[0, 90, 180, 270];
const ROTATION_LABELS: &[&str] = &["Normal", "90°", "180°", "270°"];

/// Build the controls bound to shared state.
pub fn create(ui: &Rc<DisplayUi>) -> ModeControls {
    let widget = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

    let res_dd = gtk4::DropDown::from_strings(&[]);
    let refresh_dd = gtk4::DropDown::from_strings(&[]);
    let scale_dd = gtk4::DropDown::from_strings(SCALE_LABELS);
    let rot_dd = gtk4::DropDown::from_strings(ROTATION_LABELS);
    let sync_switch = gtk4::Switch::new();

    widget.append(&row("Resolution", &res_dd));
    widget.append(&row("Refresh Rate", &refresh_dd));
    widget.append(&row("Scale", &scale_dd));
    widget.append(&row("Rotation", &rot_dd));
    widget.append(&switch_row("Adaptive Sync", &sync_switch));

    // Modes of the currently selected head, for index→mode mapping.
    let modes: Rc<RefCell<Vec<Mode>>> = Rc::new(RefCell::new(Vec::new()));

    // Name of the currently selected output.
    let selected_name = {
        let ui = ui.clone();
        Rc::new(move || {
            let sel = ui.selected.borrow();
            sel.and_then(|i| ui.heads.borrow().get(i).map(|h| h.name.clone()))
        })
    };

    // Apply the res/refresh selection: queue mode + apply, then offer revert.
    let send_selection = {
        let modes = modes.clone();
        let res_dd = res_dd.clone();
        let refresh_dd = refresh_dd.clone();
        let selected_name = selected_name.clone();
        let ui = ui.clone();
        Rc::new(move || {
            let m = modes.borrow();
            let resolutions = unique_resolutions(&m);
            let Some(&(w, h)) = resolutions.get(res_dd.selected() as usize) else {
                return;
            };
            let rates = rates_for(&m, w, h);
            // After a model swap the dropdown can report INVALID —
            // fall back to the first advertised rate instead of bailing.
            let rr_idx = if refresh_dd.selected() == gtk4::INVALID_LIST_POSITION {
                0
            } else {
                refresh_dd.selected() as usize
            };
            let Some(&rr) = rates.get(rr_idx) else {
                return;
            };
            drop(m);

            let Some(name) = selected_name() else { return };

            // Capture previous mode for the revert dialog.
            let prev = {
                let sel = ui.selected.borrow();
                sel.and_then(|i| ui.heads.borrow().get(i).map(|h| h.current_mode.clone()))
            };

            let reply = dbus_client::send_mode(&name, w, h, rr);
            eprintln!("mode {} {}x{}@{} -> {}", name, w, h, rr, reply.message);
            dbus_client::send_apply();

            // Update local head + offer revert when the mode really changed.
            if let Some(sel) = *ui.selected.borrow() {
                if let Some(head) = ui.heads.borrow_mut().get_mut(sel) {
                    head.current_mode.width = w;
                    head.current_mode.height = h;
                    head.current_mode.refresh_rate = rr;
                }
            }
            if let Some(prev) = prev {
                if prev.width != w || prev.height != h || prev.refresh_rate != rr {
                    // On revert: restore the local head's mode and resync
                    // all controls so the dropdowns show the real mode again.
                    let ui_r = ui.clone();
                    let prev_r = prev.clone();
                    let on_revert: Rc<dyn Fn()> = Rc::new(move || {
                        if let Some(sel) = *ui_r.selected.borrow() {
                            if let Some(h) = ui_r.heads.borrow_mut().get_mut(sel) {
                                h.current_mode = prev_r.clone();
                            }
                        }
                        if let Some(f) = ui_r.refresh.borrow().as_ref() {
                            f();
                        }
                    });
                    super::revert_dialog::show_mode_revert(
                        res_dd.upcast_ref(),
                        &name,
                        prev,
                        on_revert,
                    );
                }
            }
        })
    };

    // Handler registry: (widget, handler id) for safe blocking.
    let mut handlers: Vec<(gtk4::Widget, gtk4::glib::SignalHandlerId)> = Vec::new();

    // Resolution change: rebuild refresh list, then apply selection.
    {
        let modes = modes.clone();
        let refresh_dd = refresh_dd.clone();
        let send = send_selection.clone();
        let id = res_dd.connect_selected_notify(move |dd| {
            let m = modes.borrow();
            let resolutions = unique_resolutions(&m);
            let Some(&(w, h)) = resolutions.get(dd.selected() as usize) else {
                return;
            };
            let rates = rates_for(&m, w, h);
            let strings: Vec<String> = rates.iter().map(|r| hz_label(*r)).collect();
            let list =
                gtk4::StringList::new(&strings.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            refresh_dd.set_model(Some(&list));
            // Normalize the selection so send() always maps to a rate.
            refresh_dd.set_selected(0);
            drop(m);
            send();
        });
        handlers.push((res_dd.clone().upcast(), id));
    }

    // Refresh-rate change: apply selection.
    {
        let send = send_selection.clone();
        let id = refresh_dd.connect_selected_notify(move |_| send());
        handlers.push((refresh_dd.clone().upcast(), id));
    }

    // Scale change: queue + apply via daemon.
    {
        let selected_name = selected_name.clone();
        let id = scale_dd.connect_selected_notify(move |dd| {
            let Some(&scale) = SCALES.get(dd.selected() as usize) else {
                return;
            };
            let Some(name) = selected_name() else { return };
            let reply = dbus_client::send_scale(&name, scale);
            eprintln!("scale {} {} -> {}", name, scale, reply.message);
            dbus_client::send_apply();
        });
        handlers.push((scale_dd.clone().upcast(), id));
    }

    // Rotation change: queue + apply via daemon.
    {
        let selected_name = selected_name.clone();
        let id = rot_dd.connect_selected_notify(move |dd| {
            let Some(&deg) = ROTATIONS.get(dd.selected() as usize) else {
                return;
            };
            let Some(name) = selected_name() else { return };
            let reply = dbus_client::send_rotation(&name, deg);
            eprintln!("rotation {} {}° -> {}", name, deg, reply.message);
            dbus_client::send_apply();
        });
        handlers.push((rot_dd.clone().upcast(), id));
    }

    // Adaptive-sync toggle: queue + apply via daemon.
    {
        let selected_name = selected_name.clone();
        let id = sync_switch.connect_state_set(move |_, state| {
            let Some(name) = selected_name() else {
                return gtk4::glib::Propagation::Proceed;
            };
            let reply = dbus_client::send_adaptive_sync(&name, state);
            eprintln!("adaptive-sync {} {} -> {}", name, state, reply.message);
            dbus_client::send_apply();
            gtk4::glib::Propagation::Proceed
        });
        handlers.push((sync_switch.clone().upcast(), id));
    }

    let handlers = Rc::new(handlers);

    // populate rebuilds every control for the given head while all
    // change handlers are blocked, then unblocks them again.
    let populate: Rc<dyn Fn(&Head)> = Rc::new({
        let modes = modes.clone();
        move |head: &Head| {
            *modes.borrow_mut() = head.modes.clone();

            for (w, h) in handlers.iter() {
                w.block_signal(h);
            }

            // Resolutions (unique, in advertised order).
            let m = modes.borrow();
            let resolutions = unique_resolutions(&m);
            let res_strings: Vec<String> = resolutions
                .iter()
                .map(|(w, h)| format!("{}x{}", w, h))
                .collect();
            let list =
                gtk4::StringList::new(&res_strings.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            res_dd.set_model(Some(&list));
            let cur_idx = resolutions
                .iter()
                .position(|&(w, h)| w == head.current_mode.width && h == head.current_mode.height)
                .unwrap_or(0);
            res_dd.set_selected(cur_idx as u32);

            // Refresh rates for the current resolution.
            let (w, h) = resolutions.get(cur_idx).copied().unwrap_or((0, 0));
            let rates = rates_for(&m, w, h);
            let rr_strings: Vec<String> = rates.iter().map(|r| hz_label(*r)).collect();
            let list =
                gtk4::StringList::new(&rr_strings.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            refresh_dd.set_model(Some(&list));
            let rr_idx = rates
                .iter()
                .position(|&r| r == head.current_mode.refresh_rate)
                .unwrap_or(0);
            refresh_dd.set_selected(rr_idx as u32);
            drop(m);

            // Scale: nearest offered factor.
            let s_idx = SCALES
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    ((**a - head.scale).abs())
                        .partial_cmp(&(**b - head.scale).abs())
                        .unwrap()
                })
                .map(|(i, _)| i)
                .unwrap_or(1);
            scale_dd.set_selected(s_idx as u32);

            // Rotation: degrees → index.
            let r_idx = ROTATIONS
                .iter()
                .position(|&d| d == head.rotation)
                .unwrap_or(0);
            rot_dd.set_selected(r_idx as u32);

            // Adaptive sync.
            sync_switch.set_active(head.adaptive_sync);

            for (w, h) in handlers.iter() {
                w.unblock_signal(h);
            }
        }
    }) as Rc<dyn Fn(&Head)>;

    ModeControls { widget, populate }
}

/// Deduplicated (width, height) pairs preserving advertised order.
fn unique_resolutions(modes: &[Mode]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    for m in modes {
        let pair = (m.width, m.height);
        if !out.contains(&pair) {
            out.push(pair);
        }
    }
    out
}

/// Refresh rates (mHz) available for one resolution.
fn rates_for(modes: &[Mode], w: i32, h: i32) -> Vec<i32> {
    modes
        .iter()
        .filter(|m| m.width == w && m.height == h)
        .map(|m| m.refresh_rate)
        .collect()
}

/// "59.95 Hz" label for a millihertz rate.
fn hz_label(mhz: i32) -> String {
    format!("{:.2} Hz", mhz as f64 / 1000.0)
}

/// Row with a label and an existing dropdown widget.
fn row(label: &str, dd: &gtk4::DropDown) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    dd.set_halign(gtk4::Align::End);
    dd.set_hexpand(true);
    row.append(dd);
    row
}

/// Row with a label and a switch on the right.
fn switch_row(label: &str, switch: &gtk4::Switch) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    switch.set_halign(gtk4::Align::End);
    switch.set_hexpand(false);
    row.append(switch);
    row
}
