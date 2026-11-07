// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Region & Language" panel
// Copyright (C) 2026 LieraOS Project
//
// System locale configuration through systemd-localed
// (org.freedesktop.locale1) — the same polkit-authorised D-Bus
// mechanism GNOME settings use. Covers the display language
// (LANG) and the formats locale (LC_TIME/NUMERIC/MEASUREMENT).
// Keyboard layout belongs to the Mouse & Keyboard panel; the
// console keymap row shows the VC Keymap state for completeness.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::process::Command;

const BUS: &str = "org.freedesktop.locale1";
const OBJ: &str = "/org/freedesktop/locale1";

/// Locales worth showing in the language dropdown — full UTF-8 set
/// for common UI languages plus whatever is currently configured.
fn ui_locales(cur: &str) -> Vec<String> {
    let mut v: Vec<String> = Command::new("localectl")
        .arg("list-locales")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if !cur.is_empty() && !v.contains(&cur.to_string()) {
        v.push(cur.to_string());
        v.sort();
    }
    v
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let heading = gtk4::Label::new(Some(&gettext("Region & Language")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");

    // Current locale assignments, e.g. LANG=de_DE.UTF-8.
    let status = Command::new("localectl")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let lang = extract(&status, "LANG=").unwrap_or_else(|| "C.UTF-8".into());
    let formats = ["LC_TIME=", "LC_NUMERIC=", "LC_MEASUREMENT="]
        .iter()
        .find_map(|k| extract(&status, k))
        .unwrap_or_else(|| lang.clone());

    let locales = ui_locales(&lang);

    // -- Row: system language (LANG) --------------------------------------
    card.append(&locale_row(
        &gettext("Language"),
        &gettext("Menus, dialogs and messages"),
        &lang,
        &locales,
        "LANG",
    ));

    // -- Row: formats (LC_TIME + NUMERIC + MEASUREMENT) --------------------
    card.append(&locale_row(
        &gettext("Formats"),
        &gettext("Date, numbers and measurement units"),
        &formats,
        &locales,
        "LC_TIME|LC_NUMERIC|LC_MEASUREMENT",
    ));

    // -- Read-only rows: current VC/X11 keymap state -----------------------
    let vc = extract(&status, "VC Keymap:").unwrap_or_else(|| "—".into());
    let x11 = extract(&status, "X11 Layout:").unwrap_or_else(|| "—".into());
    card.append(&info_row(&gettext("Console keymap"), &vc));
    card.append(&info_row(
        &gettext("Keyboard layout"),
        &format!("{x11}  ·  {}", gettext("change under Mouse & Keyboard")),
    ));

    page.append(&card);

    let note = gtk4::Label::new(Some(&gettext(
        "Changes apply system-wide after the next login. Locale \
         writes require admin authentication.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    page.upcast()
}

/// Extract a `KEY=value` token or "Key: value" line from status text.
fn extract(status: &str, key: &str) -> Option<String> {
    for line in status.lines() {
        if let Some(pos) = line.find(key) {
            let rest = line[pos + key.len()..].trim_end();
            // Stop at the next key separator for inline lists.
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let v = rest[..end].trim().to_string();
            if !v.is_empty() && v != "(unset)" {
                return Some(v);
            }
        }
    }
    None
}

/// Boxed-list row with a locale dropdown → SetLocale via locale1.
fn locale_row(
    title: &str,
    sub: &str,
    cur: &str,
    locales: &[String],
    keys: &str,
) -> gtk4::ListBoxRow {
    let dd = gtk4::DropDown::from_strings(&locales.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = locales.iter().position(|l| l == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    let (ls, ks) = (locales.to_vec(), keys.to_string());
    dd.connect_selected_notify(move |d| {
        let Some(loc) = ls.get(d.selected() as usize) else {
            return;
        };
        // Build KEY=locale args for each locale category.
        let args: Vec<String> = ks.split('|').map(|k| format!("{k}={loc}")).collect();
        set_locale(&args);
    });
    row_widget(title, sub, dd.upcast())
}

/// Read-only info row.
fn info_row(title: &str, value: &str) -> gtk4::ListBoxRow {
    let l = gtk4::Label::new(Some(value));
    l.add_css_class("dim-label");
    l.set_valign(gtk4::Align::Center);
    row_widget(title, "", l.upcast())
}

/// Generic row: title+subtitle left, control right.
fn row_widget(title: &str, sub: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let texts = Box::new(Orientation::Vertical, 2);
    texts.set_hexpand(true);
    texts.set_valign(gtk4::Align::Center);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    if !sub.is_empty() {
        let s = gtk4::Label::new(Some(sub));
        s.add_css_class("dim-label");
        s.add_css_class("caption");
        s.set_xalign(0.0);
        texts.append(&s);
    }
    inner.append(&texts);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Write locale via org.freedesktop.locale1.SetLocale — varargs of
/// KEY=val strings plus the interactive flag (polkit prompt).
fn set_locale(assignments: &[String]) {
    let mut c = Command::new("gdbus");
    c.args([
        "call",
        "--system",
        "--dest",
        BUS,
        "--object-path",
        OBJ,
        "--method",
        "org.freedesktop.locale1.SetLocale",
    ]);
    for a in assignments {
        c.arg(a);
    }
    c.arg("true"); // interactive — allow polkit auth prompt
    let _ = c.output();
}
