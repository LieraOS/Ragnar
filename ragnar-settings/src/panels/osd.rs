// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — on-screen display rows for the Notifications
// panel (ragnar-osd configuration)
// Copyright (C) 2026 LieraOS Project
//
// Same file-based decoupling pattern as the Raven rows, but a
// separate ini (~/.config/ragnar-osd/settings.ini, [osd] section)
// so the OSD daemon keeps its own config independent from Raven's.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::Box;
use std::path::PathBuf;

use super::{common, ini};

/// ragnar-osd's settings file.
fn ini_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-osd");
    p.push("settings.ini");
    p
}

/// Write one key under [osd] via the shared, tested helper.
fn write(key: &str, value: &str) {
    let _ = ini::write_key(&ini_path(), "osd", key, value);
}

/// Append the "On-Screen Display" heading + card to the panel page.
/// The Notifications panel owns the visual slot; this module owns
/// the config plumbing.
pub fn append(page: &Box) {
    let sec = common::section(&gettext("On-Screen Display"));
    let card = common::card();
    let cfg = ini::read(&ini_path());
    let get = |k: &str| ini::get(&cfg, "osd", k);

    // Row: master switch.
    let sw = gtk4::Switch::new();
    sw.set_active(get("enabled").as_deref() != Some("false"));
    sw.set_valign(gtk4::Align::Center);
    sw.connect_state_set(|_, on| {
        write("enabled", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    card.append(&common::row(
        &gettext("Show on-screen display"),
        &gettext("Volume and brightness overlay at the bottom edge"),
        sw.upcast(),
    ));

    // Row: duration.
    let dur = common::spin(0.5, 30.0, 0.5);
    dur.set_value(
        get("duration_s")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(2.5),
    );
    dur.set_valign(gtk4::Align::Center);
    dur.connect_value_changed(|s| write("duration_s", &format!("{:.1}", s.value())));
    card.append(&common::row(
        &gettext("Show for"),
        &gettext("seconds"),
        dur.upcast(),
    ));

    // Row: size scale in percent.
    let sc = common::spin(50.0, 200.0, 10.0);
    sc.set_value(
        get("scale")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(100.0),
    );
    sc.set_valign(gtk4::Align::Center);
    sc.connect_value_changed(|s| write("scale", &format!("{:.0}", s.value())));
    card.append(&common::row(
        &gettext("Size"),
        &gettext("percent"),
        sc.upcast(),
    ));

    sec.append(&card);
    page.append(&sec);
}
