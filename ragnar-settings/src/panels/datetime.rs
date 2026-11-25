// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Date & Time" panel
// Copyright (C) 2026 LieraOS Project
//
// Real system clock + timezone configuration through the standard
// freedesktop timedated service (org.freedesktop.timedate1) —
// polkit-authorised writes, same mechanism GNOME settings use.
// NTP on/off, timezone picker, and manual set-time when NTP is off.

use super::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::process::Command;

const BUS: &str = "org.freedesktop.timedate1";
const OBJ: &str = "/org/freedesktop/timedate1";

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    let heading = gtk4::Label::new(Some(&gettext("Date & Time")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    // Live clock readout.
    let clock = gtk4::Label::new(None);
    clock.add_css_class("title-2");
    clock.set_xalign(0.0);
    tick(&clock);
    {
        let c = clock.clone();
        gtk4::glib::timeout_add_seconds_local(1, move || {
            tick(&c);
            gtk4::glib::ControlFlow::Continue
        });
    }
    page.append(&clock);

    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");

    // Row: NTP switch — SetNTP(bool, interactive).
    let ntp_sw = gtk4::Switch::new();
    ntp_sw.set_active(
        prop("NTPSynchronized").as_deref() == Some("true")
            || prop("NTP").as_deref() == Some("true"),
    );
    ntp_sw.set_valign(gtk4::Align::Center);
    ntp_sw.connect_state_set(|_, on| {
        call_bool("SetNTP", on);
        gtk4::glib::Propagation::Proceed
    });
    card.append(&row(
        &gettext("Automatic date & time"),
        &gettext("Sync via network time servers"),
        ntp_sw.upcast(),
    ));

    // Row: timezone dropdown.
    let zones = zones();
    let cur_tz = prop("Timezone").unwrap_or_default();
    let dd = gtk4::DropDown::from_strings(&zones.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = zones.iter().position(|z| *z == cur_tz) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    let zc = zones.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(z) = zc.get(d.selected() as usize) {
            call_str("SetTimezone", &[z, "true"]);
        }
    });
    card.append(&row(
        &gettext("Time zone"),
        &gettext("Region/city for local time"),
        dd.upcast(),
    ));

    // Row: manual set — only meaningful with NTP off.
    let btn = gtk4::Button::with_label(&gettext("Set manually…"));
    btn.set_valign(gtk4::Align::Center);
    btn.connect_clicked(set_dialog);
    card.append(&row(
        &gettext("Manual date & time"),
        &gettext("Only when automatic sync is off"),
        btn.upcast(),
    ));

    page.append(&card);
    page.upcast()
}

/// One boxed-list row.
fn row(title: &str, subtitle: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
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
    let s = gtk4::Label::new(Some(subtitle));
    s.add_css_class("dim-label");
    s.add_css_class("caption");
    s.set_xalign(0.0);
    texts.append(&s);
    inner.append(&texts);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Update the live clock label.
fn tick(l: &gtk4::Label) {
    let s = Command::new("date")
        .arg("+%A, %d. %B %Y  %H:%M:%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    l.set_label(&s);
}

/// All zones from `timedatectl list-timezones`.
fn zones() -> Vec<String> {
    Command::new("timedatectl")
        .arg("list-timezones")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Read a timedate1 property (strings or booleans).
fn prop(name: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            BUS,
            "--object-path",
            OBJ,
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.timedate1",
            name,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    if s.contains("true") {
        return Some("true".into());
    }
    if s.contains("false") {
        return Some("false".into());
    }
    let a = s.find('\'')? + 1;
    let b = s.rfind('\'')?;
    (b > a).then(|| s[a..b].to_string())
}

/// SetNTP(bool, interactive) via polkit D-Bus.
fn call_bool(method: &str, v: bool) {
    let _ = Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            BUS,
            "--object-path",
            OBJ,
            "--method",
            &format!("org.freedesktop.timedate1.{method}"),
            if v { "true" } else { "false" },
            "true",
        ])
        .output();
}

/// SetTimezone(string, interactive) via polkit D-Bus.
fn call_str(method: &str, args: &[&str]) {
    let mut c = Command::new("gdbus");
    c.args([
        "call",
        "--system",
        "--dest",
        BUS,
        "--object-path",
        OBJ,
        "--method",
        &format!("org.freedesktop.timedate1.{method}"),
    ]);
    for a in args {
        c.arg(a);
    }
    let _ = c.output();
}

/// Manual date/time dialog — gtk4::Calendar for the date plus an
/// hour/minute row, then SetTime(usec_utc, relative, interactive).
fn set_dialog(anchor: &gtk4::Button) {
    let win = gtk4::Window::builder()
        .title(gettext("Set date & time"))
        .modal(true)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 10);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }

    // Real calendar widget for the date part.
    let cal = gtk4::Calendar::new();
    body.append(&cal);

    let now = Command::new("date")
        .args(["+%H %M"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let v: Vec<f64> = now
        .split_whitespace()
        .filter_map(|p| p.parse().ok())
        .collect();
    let trow = Box::new(Orientation::Horizontal, 6);
    trow.set_halign(gtk4::Align::Center);
    let sh = common::spin(0.0, 23.0, 1.0);
    sh.set_value(v.first().copied().unwrap_or(0.0));
    let smi = common::spin(0.0, 59.0, 1.0);
    smi.set_value(v.get(1).copied().unwrap_or(0.0));
    trow.append(&sh);
    trow.append(&gtk4::Label::new(Some(":")));
    trow.append(&smi);
    body.append(&trow);

    let apply = gtk4::Button::with_label(&gettext("Apply"));
    apply.add_css_class("suggested-action");
    apply.set_halign(gtk4::Align::End);
    {
        let (w, c) = (win.clone(), cal.clone());
        apply.connect_clicked(move |_| {
            let dt = c.date(); // GLib::DateTime, local time
            let spec = format!(
                "{:04}-{:02}-{:02} {:02.0}:{:02.0}",
                dt.year(),
                dt.month(),
                dt.day_of_month(),
                sh.value(),
                smi.value()
            );
            if let Ok(o) = Command::new("date").args(["-d", &spec, "+%s"]).output() {
                if let Ok(secs) = String::from_utf8_lossy(&o.stdout).trim().parse::<i64>() {
                    call_str(
                        "SetTime",
                        &[&(secs * 1_000_000).to_string(), "false", "true"],
                    );
                }
            }
            w.close();
        });
    }
    body.append(&apply);
    win.set_child(Some(&body));
    win.present();
}
