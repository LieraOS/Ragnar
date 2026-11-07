// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Mouse & Keyboard" panel
// Copyright (C) 2026 LieraOS Project
//
// libinput device tuning and XKB keyboard options, written into
// the compositor's rc.xml and applied via `labwc -r` — the same
// file labwc itself reads, so there is exactly one source of
// truth and nothing is duplicated. Sections are card-style boxed
// lists matching the rest of the settings hub.

mod rc_edit;

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};

/// Common XKB layouts offered in the keyboard dropdown.
const LAYOUTS: &[(&str, &str)] = &[
    ("de", "Deutsch"),
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("fr", "Français"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("pl", "Polski"),
];

/// Whether a touchpad exists on this machine (sysfs check — no
/// libinput privileges needed).
fn has_touchpad() -> bool {
    std::fs::read_dir("/sys/class/input")
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.path().join("name");
                std::fs::read_to_string(n)
                    .map(|s| s.to_lowercase().contains("touchpad"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    // ---------------- Mouse (non-touch devices) ----------------
    page.append(&section(&gettext("Mouse"), |card| {
        card.append(&switch_row(
            &gettext("Natural scrolling"),
            "non-touch",
            "naturalScroll",
        ));
        card.append(&switch_row(
            &gettext("Left-handed"),
            "non-touch",
            "leftHanded",
        ));
        card.append(&slider_row(
            &gettext("Pointer speed"),
            "non-touch",
            "pointerSpeed",
            -1.0,
            1.0,
            0.0,
        ));
        card.append(&choice_row(
            &gettext("Acceleration profile"),
            "non-touch",
            "accelProfile",
            &[("flat", "Flat"), ("adaptive", "Adaptive")],
        ));
        card.append(&switch_row(
            &gettext("Middle button emulation"),
            "non-touch",
            "middleEmulation",
        ));
    }));

    // ---------------- Touchpad (only when present) -------------
    if has_touchpad() {
        page.append(&section(&gettext("Touchpad"), |card| {
            card.append(&switch_row(&gettext("Tap to click"), "touchpad", "tap"));
            card.append(&switch_row(
                &gettext("Natural scrolling"),
                "touchpad",
                "naturalScroll",
            ));
            card.append(&switch_row(
                &gettext("Disable while typing"),
                "touchpad",
                "disableWhileTyping",
            ));
            card.append(&slider_row(
                &gettext("Pointer speed"),
                "touchpad",
                "pointerSpeed",
                -1.0,
                1.0,
                0.0,
            ));
        }));
    }

    // ---------------- Keyboard -------------------------------
    page.append(&section(&gettext("Keyboard"), |card| {
        card.append(&layout_row());
        card.append(&slider_row(
            &gettext("Repeat rate"),
            "keyboard",
            "repeatRate",
            1.0,
            100.0,
            25.0,
        ));
        card.append(&slider_row(
            &gettext("Repeat delay (ms)"),
            "keyboard",
            "repeatDelay",
            100.0,
            1000.0,
            600.0,
        ));
        card.append(&choice_row(
            &gettext("Num lock on login"),
            "keyboard",
            "numlock",
            &[("on", "On"), ("off", "Off"), ("keep", "Keep")],
        ));
    }));

    // ---------------- Test + note -----------------------------
    let test = gtk4::Entry::builder()
        .placeholder_text(gettext("Test your keyboard here…"))
        .build();
    test.set_margin_top(6);
    page.append(&test);

    let note = gtk4::Label::new(Some(&gettext(
        "Changes are written to the compositor configuration and \
         applied immediately.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    page.upcast()
}

/// Section heading + boxed-list card filled by `fill`.
fn section<F: Fn(&ListBox)>(title: &str, fill: F) -> gtk4::Widget {
    let v = Box::new(Orientation::Vertical, 8);
    let h = gtk4::Label::new(Some(title));
    h.add_css_class("heading");
    h.set_xalign(0.0);
    v.append(&h);
    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");
    fill(&card);
    v.append(&card);
    v.upcast()
}

/// Generic row: title left, control right.
fn row(title: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 10);
    }
    let t = gtk4::Label::new(Some(title));
    t.set_hexpand(true);
    t.set_xalign(0.0);
    t.set_valign(gtk4::Align::Center);
    inner.append(&t);
    control.set_valign(gtk4::Align::Center);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// On/off mapped to yes/no — `scope` is a device category or
/// "keyboard".
fn switch_row(title: &str, scope: &str, key: &str) -> gtk4::ListBoxRow {
    let cur = get(scope, key).as_deref().unwrap_or("no") == "yes";
    let sw = gtk4::Switch::new();
    sw.set_active(cur);
    let (s, k) = (scope.to_string(), key.to_string());
    sw.connect_state_set(move |_, on| {
        set(&s, &k, if on { "yes" } else { "no" });
        gtk4::glib::Propagation::Proceed
    });
    row(title, sw.upcast())
}

/// Float slider mapped to a numeric rc key.
fn slider_row(title: &str, scope: &str, key: &str, lo: f64, hi: f64, def: f64) -> gtk4::ListBoxRow {
    let cur = get(scope, key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(def);
    let sl = gtk4::Scale::with_range(Orientation::Horizontal, lo, hi, 0.05);
    sl.set_value(cur.clamp(lo, hi));
    sl.set_size_request(200, -1);
    sl.set_draw_value(true);
    let (s, k) = (scope.to_string(), key.to_string());
    sl.connect_value_changed(move |sc| set(&s, &k, &format!("{:.2}", sc.value())));
    row(title, sl.upcast())
}

/// Dropdown mapped to a text key.
fn choice_row(title: &str, scope: &str, key: &str, opts: &[(&str, &str)]) -> gtk4::ListBoxRow {
    let labels: Vec<&str> = opts.iter().map(|o| o.1).collect();
    let dd = gtk4::DropDown::from_strings(&labels);
    let cur = get(scope, key).unwrap_or_default();
    if let Some(i) = opts.iter().position(|o| o.0 == cur) {
        dd.set_selected(i as u32);
    }
    let (s, k) = (scope.to_string(), key.to_string());
    let ids: Vec<String> = opts.iter().map(|o| o.0.to_string()).collect();
    dd.connect_selected_notify(move |d| {
        if let Some(v) = ids.get(d.selected() as usize) {
            set(&s, &k, v);
        }
    });
    row(title, dd.upcast())
}

/// XKB layout dropdown (writes `<layout>` inside <keyboard>).
fn layout_row() -> gtk4::ListBoxRow {
    let labels: Vec<&str> = LAYOUTS.iter().map(|o| o.1).collect();
    let dd = gtk4::DropDown::from_strings(&labels);
    let cur = rc_edit::get_keyboard("layout").unwrap_or_else(|| "de".into());
    if let Some(i) = LAYOUTS.iter().position(|o| o.0 == cur) {
        dd.set_selected(i as u32);
    }
    dd.connect_selected_notify(|d| {
        if let Some((id, _)) = LAYOUTS.get(d.selected() as usize) {
            rc_edit::set_keyboard("layout", id);
            rc_edit::reconfigure();
        }
    });
    row(&gettext("Keyboard layout"), dd.upcast())
}

/// Read helper: "keyboard" scope → keyboard block else device.
fn get(scope: &str, key: &str) -> Option<String> {
    if scope == "keyboard" {
        rc_edit::get_keyboard(key)
    } else {
        rc_edit::get_device(scope, key)
    }
}

/// Write helper + live reload.
fn set(scope: &str, key: &str, val: &str) {
    if scope == "keyboard" {
        rc_edit::set_keyboard(key, val);
    } else {
        rc_edit::set_device(scope, key, val);
    }
    rc_edit::reconfigure();
}
