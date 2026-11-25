// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings-gui — appearance sub-page: filename effects
// Copyright (C) 2026 LieraOS Project
//
// Optional animation for file and folder names — a brightness
// pulse or a two-colour wave travelling across the glyphs.
// Backed by ~/.config/ragnar-settings/font-effects.ini [names];
// every write goes through ini::write_key so hand-edited keys and
// comments survive. The renderers (valkyrie, ragnar-view desktop
// icons) watch the file and apply changes live. Default: off.

use crate::i18n::msgid;
use crate::panels::ini;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::glib;
use std::path::PathBuf;

const SECTION: &str = "names";

const MODE_OPTIONS: &[(&str, &str)] = &[
    (msgid("Pulse"), "pulse"),
    (msgid("Colour wave"), "gradient"),
];

const DIR_OPTIONS: &[(&str, &str)] = &[
    (msgid("Left to right"), "ltr"),
    (msgid("Right to left"), "rtl"),
    (msgid("Top to bottom"), "ttb"),
    (msgid("Bottom to top"), "btt"),
];

fn path() -> PathBuf {
    ini::config_path("ragnar-settings/font-effects.ini")
}

fn save(key: &str, value: &str) {
    let _ = ini::write_key(&path(), SECTION, key, value);
}

/// Build the effects group appended to the fonts page.
pub fn build() -> gtk4::Box {
    let map = ini::read(&path());
    let get = |k: &str, d: &str| ini::get(&map, SECTION, k).unwrap_or_else(|| d.to_string());

    let group = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    group.set_margin_top(18);
    let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep.set_margin_bottom(6);
    group.append(&sep);
    let head = gtk4::Label::new(Some(&gettext("Filename effects")));
    head.add_css_class("heading");
    head.set_xalign(0.0);
    group.append(&head);
    let hint = gtk4::Label::new(Some(&gettext(
        "Applies to file and folder names in Valkyrie and on the desktop. Off by default.",
    )));
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    group.append(&hint);

    // Master switch.
    let sw_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sw_row.append(&gtk4::Label::new(Some(&gettext("Animate names"))));
    let sw = gtk4::Switch::new();
    sw.set_halign(gtk4::Align::End);
    sw.set_hexpand(true);
    sw.set_state(get("enabled", "false") == "true");
    sw.connect_state_set(|_, on| {
        save("enabled", if on { "true" } else { "false" });
        glib::Propagation::Proceed
    });
    sw_row.append(&sw);
    group.append(&sw_row);

    // Effect type + direction dropdowns (string enums -> ini).
    group.append(&dd_row(
        &gettext("Effect"),
        "mode",
        &get("mode", "pulse"),
        MODE_OPTIONS,
    ));
    group.append(&dd_row(
        &gettext("Direction"),
        "direction",
        &get("direction", "ltr"),
        DIR_OPTIONS,
    ));

    // Colour pickers -> #rrggbb.
    group.append(&color_row(&gettext("Base colour"), "color1", &get("color1", "#ffffff")));
    group.append(&color_row(
        &gettext("Wave colour"),
        "color2",
        &get("color2", "#4d99ff"),
    ));

    // Strength + speed sliders.
    group.append(&scale_row(
        &gettext("Strength"),
        "strength",
        get("strength", "0.3").parse().unwrap_or(0.3),
        0.0,
        1.0,
        0.05,
    ));
    group.append(&scale_row(
        &gettext("Speed"),
        "speed",
        get("speed", "1.0").parse().unwrap_or(1.0),
        0.2,
        4.0,
        0.1,
    ));

    group
}

/// Label + DropDown bound to a string-enum ini key.
fn dd_row(
    label: &str,
    key: &'static str,
    current: &str,
    options: &'static [(&'static str, &'static str)],
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    let labels: Vec<String> = options.iter().map(|(l, _)| gettext(*l)).collect();
    let lrefs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let dd = gtk4::DropDown::from_strings(&lrefs);
    dd.set_halign(gtk4::Align::End);
    if let Some(i) = options.iter().position(|(_, v)| *v == current) {
        dd.set_selected(i as u32);
    }
    dd.connect_selected_notify(move |d| {
        let (_, value) = options[d.selected() as usize];
        save(key, value);
    });
    row.append(&dd);
    row
}

/// Label + ColorDialogButton; the chosen colour is stored as #rrggbb.
fn color_row(label: &str, key: &'static str, current: &str) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    let btn = gtk4::ColorDialogButton::new(Some(gtk4::ColorDialog::new()));
    btn.set_halign(gtk4::Align::End);
    btn.set_hexpand(true);
    if let Ok(rgba) = gtk4::gdk::RGBA::parse(current) {
        btn.set_rgba(&rgba);
    }
    btn.connect_rgba_notify(move |b| {
        let c = b.rgba();
        save(
            key,
            &format!(
                "#{:02x}{:02x}{:02x}",
                (c.red() * 255.0).round() as u8,
                (c.green() * 255.0).round() as u8,
                (c.blue() * 255.0).round() as u8
            ),
        );
    });
    row.append(&btn);
    row
}

/// Label + Scale writing a numeric ini key on change.
fn scale_row(
    label: &str,
    key: &'static str,
    current: f64,
    min: f64,
    max: f64,
    step: f64,
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, step);
    scale.set_value(current);
    scale.set_hexpand(true);
    scale.set_digits(2);
    scale.set_value_pos(gtk4::PositionType::Right);
    scale.connect_value_changed(move |s| save(key, &format!("{:.2}", s.value())));
    row.append(&scale);
    row
}
