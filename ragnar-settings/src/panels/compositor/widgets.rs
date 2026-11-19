// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor panel: shared row builders
// Copyright (C) 2026 LieraOS Project
//
// Small builders for the three control shapes the sections use:
// a yes/no switch, an integer spin button and a labelled dropdown —
// each wired to read its current value from rc.xml and to write +
// reload on change.

use super::super::common;
use crate::panels::rcxml;
use gettextrs::gettext;
use gtk4::prelude::*;
use std::rc::Rc;

/// Read the current `<section><key>` value as yes/no.
fn cur_bool(section: &str, key: &str, default: bool) -> bool {
    rcxml::get_section(section, key)
        .map(|v| matches!(v.trim(), "yes" | "true" | "1"))
        .unwrap_or(default)
}

/// Write one value and reload the compositor.
fn apply(write: impl Fn()) {
    write();
    rcxml::reload();
}

/// Switch row bound to a `<section><key>yes|no</key>` element.
pub fn switch_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    section: &'static str,
    key: &'static str,
    default: bool,
) {
    let sw = gtk4::Switch::builder()
        .active(cur_bool(section, key, default))
        .valign(gtk4::Align::Center)
        .build();
    sw.connect_state_notify(move |s| {
        apply(|| rcxml::set_section(section, key, if s.is_active() { "yes" } else { "no" }));
    });
    card.append(&common::row(title, subtitle, sw.upcast()));
}

/// Integer spin row bound to a `<section><key>N</key>` element.
pub fn spin_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    section: &'static str,
    key: &'static str,
    range: (f64, f64),
    default: f64,
) {
    let cur = rcxml::get_section(section, key)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default);
    let spin = common::spin(range.0, range.1, 1.0);
    spin.set_value(cur);
    spin.set_valign(gtk4::Align::Center);
    spin.connect_value_changed(move |s| {
        let v = s.value() as i64;
        apply(|| rcxml::set_section(section, key, &v.to_string()));
    });
    card.append(&common::row(title, subtitle, spin.upcast()));
}

/// Integer spin row bound to an XML attribute `attr` on `<tag>`
/// (e.g. `<desktops number="4">`).
pub fn attr_spin_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    tag: &'static str,
    attr: &'static str,
    range: (f64, f64),
    default: f64,
) {
    let cur = rcxml::get_attr(tag, attr)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default);
    let spin = common::spin(range.0, range.1, 1.0);
    spin.set_value(cur);
    spin.set_valign(gtk4::Align::Center);
    spin.connect_value_changed(move |s| {
        let v = s.value() as i64;
        apply(|| rcxml::set_attr(tag, attr, &v.to_string()));
    });
    card.append(&common::row(title, subtitle, spin.upcast()));
}

/// Dropdown row bound to a `<section><key>choice</key>` element.
/// `options` are (id, translated label) pairs in display order.
pub fn choice_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    section: &'static str,
    key: &'static str,
    options: &'static [(&'static str, &'static str)],
    default: &'static str,
) {
    let cur = rcxml::get_section(section, key).unwrap_or_else(|| default.into());
    let sel = options
        .iter()
        .position(|(id, _)| *id == cur.trim())
        .unwrap_or(0) as u32;
    let labels: Vec<String> = options.iter().map(|(_, l)| gettext(*l)).collect();
    let dd = gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    dd.set_selected(sel);
    dd.set_valign(gtk4::Align::Center);
    dd.connect_selected_notify(move |d| {
        if let Some((id, _)) = options.get(d.selected() as usize) {
            let id = (*id).to_string();
            apply(move || rcxml::set_section(section, key, &id));
        }
    });
    card.append(&common::row(title, subtitle, dd.upcast()));
}

/// Text entry row bound to an attribute on an element — needed for
/// attribute-carrying settings like
/// `<font place="ActiveWindow" name="…"/>` where several same-tag
/// elements exist and `tag_expr` disambiguates. Writes on Enter or
/// focus-out.
pub fn attr_entry_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    tag_expr: &'static str,
    attr: &'static str,
    default: &'static str,
) {
    let cur = rcxml::get_attr(tag_expr, attr).unwrap_or_else(|| default.into());
    let entry = gtk4::Entry::builder()
        .text(&cur)
        .valign(gtk4::Align::Center)
        .build();
    let write = Rc::new(move |text: String| {
        apply(move || rcxml::set_attr(tag_expr, attr, &text));
    });
    {
        let write = write.clone();
        entry.connect_activate(move |e| write(e.text().to_string()));
    }
    {
        let write = write.clone();
        let ctrl = gtk4::EventControllerFocus::new();
        let e = entry.clone();
        ctrl.connect_leave(move |_| write(e.text().to_string()));
        entry.add_controller(ctrl);
    }
    card.append(&common::row(title, subtitle, entry.upcast()));
}

/// A locked feature row: the control name plus why it cannot be
/// used — rendered insensitive so it reads as present-but-blocked.
pub fn locked_row(card: &gtk4::ListBox, title: &str, cap: crate::backend::compositor::Cap) {
    let supported = crate::backend::compositor::supports(cap);
    let note = if supported {
        gettext("Not configurable yet — support is on the way.")
    } else {
        gettext("Requires the Ragnar compositor.")
    };
    let icon = gtk4::Image::from_icon_name("emblem-readonly-symbolic");
    icon.set_valign(gtk4::Align::Center);
    let row = common::row(title, &note, icon.upcast());
    row.set_sensitive(false);
    card.append(&row);
}
