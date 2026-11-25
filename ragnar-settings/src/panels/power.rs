// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Power" panel
// Copyright (C) 2026 LieraOS Project
//
// Power-profile switching through the net.hadess.PowerProfiles
// D-Bus API — provided by power-profiles-daemon or tuned-ppd
// alike, so the panel works with either backend without knowing
// which is installed. Battery state comes from UPower's
// DisplayDevice. The battery section only renders when a real
// battery exists — desktops show just the profile card, never a
// dead widget.

use crate::i18n::msgid;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{gio, glib, Box, ListBox, Orientation};
use std::process::Command;

use super::common;

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    // -- Power profile ------------------------------------------------------
    // Profiles are discovered live — the daemon only exposes what
    // the platform driver supports (this system: saver+balanced;
    // laptops with amd_pstate/intel_pstate also get performance).
    let prof = section(&gettext("Power profile"));
    let card = card_list();
    let profiles = available_profiles();
    if profiles.is_empty() {
        card.append(&missing_row(
            &gettext("Not available"),
            &gettext("No power-profile service is installed (power-profiles-daemon or tuned-ppd)."),
        ));
    } else {
        let group = gtk4::CheckButton::new();
        for (id, active) in profiles {
            let (title, sub) = profile_label(&id);
            let cb = gtk4::CheckButton::new();
            cb.set_group(Some(&group));
            cb.set_active(active);
            let idc = id.clone();
            cb.connect_toggled(move |b| {
                if b.is_active() {
                    set_profile(&idc);
                }
            });
            card.append(&radio_row(&title, &sub, &cb));
        }
    }
    prof.append(&card);
    page.append(&prof);

    // -- Suspend & screen ---------------------------------------------------
    // idle-delay is consumed live by ragnar-daemon's screenlock
    // service — it rebuilds the swayidle command on change. The
    // suspend rows only appear when the gsd-power schema exists
    // (gnome-settings-daemon is not part of Ragnar).
    let sus = section(&gettext("Screen & sleep"));
    let sc = card_list();
    sc.append(&choice_row(
        &gettext("Blank screen after"),
        "org.gnome.desktop.session",
        "idle-delay",
        &[
            (0, "Never"),
            (60, "1 minute"),
            (120, "2 minutes"),
            (300, "5 minutes"),
            (600, "10 minutes"),
            (900, "15 minutes"),
        ],
    ));
    if let Some(r) = suspend_row(&gettext("Suspend on AC power"), "ac") {
        sc.append(&r);
    }
    if battery().is_some() {
        if let Some(r) = suspend_row(&gettext("Suspend on battery"), "battery") {
            sc.append(&r);
        }
    }
    sus.append(&sc);
    page.append(&sus);

    // -- Power dialog -------------------------------------------------------
    // Which action tiles ragnar-power-dialog offers. The keys live
    // in the org.lieraos.ragnar.session schema shipped by the
    // ragnar-power-dialog package — the section only renders when
    // the schema is installed.
    if let Some(sess) = session_settings() {
        let dlg = section(&gettext("Power dialog"));
        let dc = card_list();
        for (key, label) in [
            ("show-lock", msgid("Lock screen")),
            ("show-logout", msgid("Log out")),
            ("show-suspend", msgid("Suspend")),
            ("show-hibernate", msgid("Hibernate")),
            ("show-reboot", msgid("Restart")),
            ("show-shutdown", msgid("Shut down")),
        ] {
            dc.append(&switch_row(&sess, key, &gettext(label)));
        }
        dlg.append(&dc);
        page.append(&dlg);
    }

    // -- Battery (only when one exists) ------------------------------------
    if let Some(info) = battery() {
        let bat = section(&gettext("Battery"));
        let bc = card_list();
        for (label, val) in info {
            bc.append(&info_row(&label, &val));
        }
        bat.append(&bc);
        page.append(&bat);
    }

    page.upcast()
}

/// Proxy for net.hadess.PowerProfiles on the system bus. The name
/// is owned by power-profiles-daemon or by tuned-ppd (which
/// emulates the same API on top of tuned) — D-Bus activation starts
/// whichever service file exists.
fn ppd_proxy() -> Option<gio::DBusProxy> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        "net.hadess.PowerProfiles",
        "/net/hadess/PowerProfiles",
        "net.hadess.PowerProfiles",
        gio::Cancellable::NONE,
    )
    .ok()
}

/// Read Profiles + ActiveProfile → [(id, is_active)]. The service
/// only advertises profiles the platform driver actually supports.
fn available_profiles() -> Vec<(String, bool)> {
    use std::collections::HashMap;
    let Some(p) = ppd_proxy() else {
        return Vec::new();
    };
    let res = p
        .call_sync(
            "org.freedesktop.DBus.Properties.GetAll",
            Some(&("net.hadess.PowerProfiles",).to_variant()),
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        )
        .ok();
    let Some(props) = res
        .and_then(|v| v.get::<(HashMap<String, glib::Variant>,)>())
        .map(|(p,)| p)
    else {
        return Vec::new();
    };
    let active = props
        .get("ActiveProfile")
        .and_then(|v| v.get::<String>())
        .unwrap_or_default();
    props
        .get("Profiles")
        .and_then(|v| v.get::<Vec<HashMap<String, glib::Variant>>>())
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d.get("Profile").and_then(|v| v.get::<String>()))
        .map(|name| (name.clone(), name == active))
        .collect()
}

/// Write ActiveProfile — user-callable by design of the API, no
/// elevation needed.
fn set_profile(id: &str) {
    if let Some(p) = ppd_proxy() {
        let _ = p.call_sync(
            "org.freedesktop.DBus.Properties.Set",
            Some(
                &(
                    "net.hadess.PowerProfiles",
                    "ActiveProfile",
                    glib::Variant::from(id),
                )
                    .to_variant(),
            ),
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        );
    }
}

/// Title + description per known profile id.
fn profile_label(id: &str) -> (String, String) {
    match id {
        "power-saver" => (
            gettext("Power saver"),
            gettext("Lower performance, longer battery life"),
        ),
        "balanced" => (
            gettext("Balanced"),
            gettext("Standard performance and power usage"),
        ),
        "performance" => (
            gettext("Performance"),
            gettext("Highest performance, higher power draw"),
        ),
        other => (other.to_string(), String::new()),
    }
}

/// UPower DisplayDevice details → labelled rows, or None on a
/// machine without a battery.
fn battery() -> Option<Vec<(String, String)>> {
    let out = Command::new("upower")
        .args(["-i", "/org/freedesktop/UPower/devices/DisplayDevice"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    let get = |k: &str| -> Option<String> {
        s.lines()
            .find(|l| l.trim_start().starts_with(k))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
    };
    let state = get("state")?;
    if state == "unknown" && get("percentage").is_none() {
        return None; // desktop without battery
    }
    let mut rows = vec![(
        gettext("Charge level"),
        get("percentage").unwrap_or_else(|| "—".into()),
    )];
    rows.push((
        gettext("State"),
        match state.as_str() {
            "charging" => gettext("Charging"),
            "discharging" => gettext("Discharging"),
            "fully-charged" => gettext("Fully charged"),
            _ => state.clone(),
        },
    ));
    if let Some(t) = get("time to empty") {
        rows.push((gettext("Time remaining"), t));
    }
    if let Some(t) = get("time to full") {
        rows.push((gettext("Time to full"), t));
    }
    if let Some(h) = get("capacity") {
        rows.push((gettext("Battery health"), h));
    }
    Some(rows)
}

/// Section heading + content container.
fn section(title: &str) -> Box {
    let v = Box::new(Orientation::Vertical, 8);
    let h = gtk4::Label::new(Some(title));
    h.add_css_class("heading");
    h.set_xalign(0.0);
    v.append(&h);
    v
}

/// Card-style list.
fn card_list() -> ListBox {
    let l = ListBox::new();
    l.set_selection_mode(gtk4::SelectionMode::None);
    l.add_css_class("boxed-list");
    l
}

/// Radio row: check button + title/subtitle.
fn radio_row(title: &str, sub: &str, cb: &gtk4::CheckButton) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    inner.append(cb);
    let texts = Box::new(Orientation::Vertical, 2);
    texts.set_valign(gtk4::Align::Center);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    let s = gtk4::Label::new(Some(sub));
    s.add_css_class("dim-label");
    s.add_css_class("caption");
    s.set_xalign(0.0);
    texts.append(&s);
    inner.append(&texts);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Seconds-choice row writing a uint gsettings key (idle-delay is
/// type `u` — set_int would fail silently and revert the value).
fn choice_row(title: &str, schema: &str, key: &str, opts: &[(i64, &str)]) -> gtk4::ListBoxRow {
    let Some(s) = common::schema_lookup(schema, key) else {
        return missing_row(
            title,
            &gettext("Unavailable: the settings service is not installed."),
        );
    };
    let cur = s.uint(key) as i64;
    let dd = gtk4::DropDown::from_strings(&opts.iter().map(|o| o.1).collect::<Vec<_>>());
    if let Some(i) = opts.iter().position(|o| o.0 == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    let ids: Vec<i64> = opts.iter().map(|o| o.0).collect();
    let (sc, k) = (schema.to_string(), key.to_string());
    dd.connect_selected_notify(move |d| {
        if let Some(v) = ids.get(d.selected() as usize) {
            let _ = gio::Settings::new(&sc).set_uint(&k, *v as u32);
        }
    });
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let l = gtk4::Label::new(Some(title));
    l.set_hexpand(true);
    l.set_xalign(0.0);
    inner.append(&l);
    inner.append(&dd);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Suspend-timeout row — sets type=suspend/nothing + timeout secs.
/// The gsd-power schema only exists when gnome-settings-daemon is
/// installed; without it the row is skipped entirely (no consumer
/// would read the keys anyway).
fn suspend_row(title: &str, rail: &str) -> Option<gtk4::ListBoxRow> {
    const OPTS: &[(i64, &str)] = &[
        (0, "Never"),
        (900, "15 minutes"),
        (1800, "30 minutes"),
        (3600, "1 hour"),
        (7200, "2 hours"),
    ];
    let schema = "org.gnome.settings-daemon.plugins.power";
    let tkey = format!("sleep-inactive-{rail}-timeout");
    let ykey = format!("sleep-inactive-{rail}-type");
    let s = common::schema_lookup(schema, &tkey)?;
    common::schema_lookup(schema, &ykey)?;
    let cur = if s.string(&ykey).as_str() == "suspend" {
        s.int(&tkey) as i64
    } else {
        0
    };
    let dd = gtk4::DropDown::from_strings(&OPTS.iter().map(|o| o.1).collect::<Vec<_>>());
    if let Some(i) = OPTS.iter().position(|o| o.0 == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    dd.connect_selected_notify(move |d| {
        let Some(v) = OPTS.get(d.selected() as usize).map(|o| o.0) else {
            return;
        };
        let s = gio::Settings::new(schema);
        let _ = s.set_string(&ykey, if v > 0 { "suspend" } else { "nothing" });
        let _ = s.set_int(&tkey, v as i32);
    });
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let l = gtk4::Label::new(Some(title));
    l.set_hexpand(true);
    l.set_xalign(0.0);
    inner.append(&l);
    inner.append(&dd);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    Some(r)
}

/// Settings for org.lieraos.ragnar.session — None when the schema
/// is not installed (ragnar-power-dialog package missing), so the
/// section is skipped instead of writing into a void.
fn session_settings() -> Option<gio::Settings> {
    const SCHEMA: &str = "org.lieraos.ragnar.session";
    gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(SCHEMA, true))
        .map(|_| gio::Settings::new(SCHEMA))
}

/// Boolean switch row bound to a gsettings key of `s`.
fn switch_row(s: &gio::Settings, key: &str, title: &str) -> gtk4::ListBoxRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    sw.set_active(s.boolean(key));
    let (s, k) = (s.clone(), key.to_string());
    sw.connect_state_set(move |_, on| {
        let _ = s.set_boolean(&k, on);
        glib::Propagation::Proceed
    });
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let l = gtk4::Label::new(Some(title));
    l.set_hexpand(true);
    l.set_xalign(0.0);
    inner.append(&l);
    inner.append(&sw);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Insensitive row used when a backing service or schema is
/// absent — explains why instead of rendering a dead control.
fn missing_row(title: &str, why: &str) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let texts = Box::new(Orientation::Vertical, 2);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    let s = gtk4::Label::new(Some(why));
    s.add_css_class("dim-label");
    s.add_css_class("caption");
    s.set_xalign(0.0);
    s.set_wrap(true);
    texts.append(&s);
    inner.append(&texts);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r.set_sensitive(false);
    r
}

/// Read-only info row.
fn info_row(label: &str, value: &str) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let l = gtk4::Label::new(Some(label));
    l.set_hexpand(true);
    l.set_xalign(0.0);
    inner.append(&l);
    let v = gtk4::Label::new(Some(value));
    v.add_css_class("dim-label");
    inner.append(&v);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}
