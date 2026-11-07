// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Notifications" panel (Raven configuration)
// Copyright (C) 2026 LieraOS Project
//
// Writes ragnar-raven's own settings.ini — the file is the
// interface, Raven stays fully decoupled: enable/disable, slide-in
// duration, and the monitor the notification appears on (connector
// names from DisplayServices, same source as the display and
// wallpaper panels). The notification sound itself is configured
// in the System Sounds panel.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::display::dbus_client;

/// Raven's own settings file.
fn ini_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-raven");
    p.push("settings.ini");
    p
}

/// Build the notifications panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let cfg = read_ini();

    let heading = gtk4::Label::new(Some(&gettext("Notifications")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    // Card-style grouped list (Adwaita boxed-list look).
    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");

    // Row: master switch.
    let sw = gtk4::Switch::new();
    sw.set_active(get(&cfg, "enabled").as_deref() != Some("false"));
    sw.set_valign(gtk4::Align::Center);
    sw.connect_state_set(|_, on| {
        write_key("enabled", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    card.append(&row(
        &gettext("Show notifications"),
        &gettext("Slide in alerts on the right edge of the screen"),
        sw.upcast(),
    ));

    // Row: duration.
    let dur = gtk4::SpinButton::with_range(2.0, 120.0, 1.0);
    dur.set_value(
        get(&cfg, "duration_s")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(18.0),
    );
    dur.set_valign(gtk4::Align::Center);
    dur.connect_value_changed(|s| write_key("duration_s", &format!("{:.0}", s.value())));
    card.append(&row(
        &gettext("Show for"),
        &gettext("seconds"),
        dur.upcast(),
    ));

    // Row: monitor choice.
    let mut names = vec!["all".to_string()];
    names.extend(
        dbus_client::fetch_outputs()
            .iter()
            .filter(|h| h.enabled)
            .map(|h| h.name.clone()),
    );
    let cur = get(&cfg, "monitor").unwrap_or_else(|| "all".into());
    let dd = gtk4::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = names.iter().position(|n| *n == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    let names_c = names.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(n) = names_c.get(d.selected() as usize) {
            write_key("monitor", n);
        }
    });
    card.append(&row(
        &gettext("Show on monitor"),
        &gettext("\"all\" shows the alert on every screen"),
        dd.upcast(),
    ));

    page.append(&card);

    // ----------------------------------------------------------- //
    // On-screen display — same file-based pattern, own ini:        //
    // ~/.config/ragnar-osd/settings.ini [osd]                      //
    // ----------------------------------------------------------- //
    let osd_heading = gtk4::Label::new(Some(&gettext("On-Screen Display")));
    osd_heading.add_css_class("heading");
    osd_heading.set_xalign(0.0);
    page.append(&osd_heading);

    let osd_card = ListBox::new();
    osd_card.set_selection_mode(gtk4::SelectionMode::None);
    osd_card.add_css_class("boxed-list");
    let ocfg = osd_read_ini();

    // Row: master switch.
    let osw = gtk4::Switch::new();
    osw.set_active(osd_get(&ocfg, "enabled").as_deref() != Some("false"));
    osw.set_valign(gtk4::Align::Center);
    osw.connect_state_set(|_, on| {
        osd_write_key("enabled", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    osd_card.append(&row(
        &gettext("Show on-screen display"),
        &gettext("Volume and brightness overlay at the bottom edge"),
        osw.upcast(),
    ));

    // Row: duration.
    let odur = gtk4::SpinButton::with_range(0.5, 30.0, 0.5);
    odur.set_value(
        osd_get(&ocfg, "duration_s")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(2.5),
    );
    odur.set_valign(gtk4::Align::Center);
    odur.connect_value_changed(|s| osd_write_key("duration_s", &format!("{:.1}", s.value())));
    osd_card.append(&row(
        &gettext("Show for"),
        &gettext("seconds"),
        odur.upcast(),
    ));

    // Row: size scale in percent.
    let osc = gtk4::SpinButton::with_range(50.0, 200.0, 10.0);
    osc.set_value(
        osd_get(&ocfg, "scale")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(100.0),
    );
    osc.set_valign(gtk4::Align::Center);
    osc.connect_value_changed(|s| osd_write_key("scale", &format!("{:.0}", s.value())));
    osd_card.append(&row(&gettext("Size"), &gettext("percent"), osc.upcast()));

    page.append(&osd_card);

    let note = gtk4::Label::new(Some(&gettext(
        "Notification sound lives in System Sounds; history and \
         appointment reminders are handled by Raven itself.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    page.upcast()
}

/// One boxed-list row: title + subtitle on the left, control right.
fn row(title: &str, subtitle: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let texts = Box::new(Orientation::Vertical, 2);
    texts.set_valign(gtk4::Align::Center);
    texts.set_hexpand(true);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    if !subtitle.is_empty() {
        let s = gtk4::Label::new(Some(subtitle));
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

/// Read [notifications] section into key → value.
fn read_ini() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut in_sec = false;
    if let Ok(s) = std::fs::read_to_string(ini_path()) {
        for l in s.lines() {
            let t = l.trim();
            if t.starts_with('[') {
                in_sec = t == "[notifications]";
            } else if in_sec {
                if let Some((k, v)) = t.split_once('=') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }
    map
}

fn get(m: &BTreeMap<String, String>, key: &str) -> Option<String> {
    m.get(key).cloned()
}

// ------------------------------------------------------------------ //
// OSD helpers — same INI pattern, but ragnar-osd's own file and      //
// [osd] section so the daemon stays decoupled from Raven's config.   //
// ------------------------------------------------------------------ //

/// ragnar-osd's settings file.
fn osd_ini_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-osd");
    p.push("settings.ini");
    p
}

/// Read the [osd] section into key → value.
fn osd_read_ini() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut in_sec = false;
    if let Ok(s) = std::fs::read_to_string(osd_ini_path()) {
        for l in s.lines() {
            let t = l.trim();
            if t.starts_with('[') {
                in_sec = t == "[osd]";
            } else if in_sec {
                if let Some((k, v)) = t.split_once('=') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }
    map
}

fn osd_get(m: &BTreeMap<String, String>, key: &str) -> Option<String> {
    m.get(key).cloned()
}

/// Write one key under [osd], preserving everything else.
fn osd_write_key(key: &str, value: &str) {
    let path = osd_ini_path();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let mut in_sec = false;
    let mut done = false;
    let mut insert_at: Option<usize> = None;
    for i in 0..lines.len() {
        let t = lines[i].trim().to_string();
        if t.starts_with('[') {
            if in_sec && !done {
                insert_at = Some(i);
            }
            in_sec = t == "[osd]";
        } else if in_sec {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    lines[i] = format!("{key} = {value}");
                    done = true;
                }
            }
        }
    }
    if !done {
        if let Some(i) = insert_at {
            lines.insert(i, format!("{key} = {value}"));
        } else {
            if !lines.iter().any(|l| l.trim() == "[osd]") {
                lines.push("[osd]".to_string());
            }
            lines.push(format!("{key} = {value}"));
        }
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}

/// Write one key under [notifications], preserving everything else.
fn write_key(key: &str, value: &str) {
    let path = ini_path();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let mut in_sec = false;
    let mut done = false;
    let mut insert_at: Option<usize> = None;
    for i in 0..lines.len() {
        let t = lines[i].trim().to_string();
        if t.starts_with('[') {
            if in_sec && !done {
                insert_at = Some(i);
            }
            in_sec = t == "[notifications]";
        } else if in_sec {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    lines[i] = format!("{key} = {value}");
                    done = true;
                }
            }
        }
    }
    if !done {
        if let Some(i) = insert_at {
            lines.insert(i, format!("{key} = {value}"));
        } else {
            if !lines.iter().any(|l| l.trim() == "[notifications]") {
                lines.push("[notifications]".to_string());
            }
            lines.push(format!("{key} = {value}"));
        }
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}
