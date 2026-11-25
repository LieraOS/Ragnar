// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Notifications" panel (Raven configuration)
// Copyright (C) 2026 LieraOS Project
//
// Writes ragnar-raven's own settings.ini — the file is the
// interface, Raven stays fully decoupled: enable/disable, slide-in
// duration, the monitor the notification appears on (connector
// names from DisplayServices, same source as the display and
// wallpaper panels), history retention, the event sources Raven
// watches and the per-category accent colours. The notification
// sound itself is configured in the System Sounds panel.

use crate::i18n::msgid;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::path::PathBuf;

use super::display::dbus_client;
use super::{common, ini, osd};

/// Raven's own settings file.
fn ini_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-raven");
    p.push("settings.ini");
    p
}

/// Write one key under [notifications] via the shared, tested
/// helper (private writers were removed: one produced headers
/// without ']' and corrupted the whole ini).
fn write(key: &str, value: &str) {
    let _ = ini::write_key(&ini_path(), "notifications", key, value);
}

/// Build the notifications panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Notifications"));
    let cfg = ini::read(&ini_path());
    let get = |k: &str| ini::get(&cfg, "notifications", k);

    // ----------------------------------------------------------- //
    // Card 1: display behaviour                                    //
    // ----------------------------------------------------------- //
    let card = common::card();

    // Row: master switch.
    let sw = gtk4::Switch::new();
    sw.set_active(get("enabled").as_deref() != Some("false"));
    sw.set_valign(gtk4::Align::Center);
    sw.connect_state_set(|_, on| {
        write("enabled", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    card.append(&common::row(
        &gettext("Show notifications"),
        &gettext("Slide in alerts on the right edge of the screen"),
        sw.upcast(),
    ));

    // Row: duration.
    let dur = common::spin(2.0, 120.0, 1.0);
    dur.set_value(
        get("duration_s")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(18.0),
    );
    dur.set_valign(gtk4::Align::Center);
    dur.connect_value_changed(|s| write("duration_s", &format!("{:.0}", s.value())));
    card.append(&common::row(
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
    let cur = get("monitor").unwrap_or_else(|| "all".into());
    let dd = gtk4::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = names.iter().position(|n| *n == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    let names_c = names.clone();
    dd.connect_selected_notify(move |d| {
        if let Some(n) = names_c.get(d.selected() as usize) {
            write("monitor", n);
        }
    });
    card.append(&common::row(
        &gettext("Show on monitor"),
        &gettext("\"all\" shows the alert on every screen"),
        dd.upcast(),
    ));

    // Row: on-screen position — panel gsettings key, not part of
    // Raven's ini. Only shown when the schema exists.
    if let Some(panel) = common::schema_lookup("org.lieraos.ragnar.panel", "notification-position")
    {
        const POS: &[(&str, &str)] = &[
            ("RAGNAR_NOTIFICATION_POSITION_TOP_LEFT", msgid("Top left")),
            ("RAGNAR_NOTIFICATION_POSITION_TOP_RIGHT", msgid("Top right")),
            (
                "RAGNAR_NOTIFICATION_POSITION_BOTTOM_LEFT",
                msgid("Bottom left"),
            ),
            (
                "RAGNAR_NOTIFICATION_POSITION_BOTTOM_RIGHT",
                msgid("Bottom right"),
            ),
        ];
        let labels: Vec<String> = POS.iter().map(|o| gettext(o.1)).collect();
        let pdd =
            gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
        let cur = panel.string("notification-position").to_string();
        if let Some(i) = POS.iter().position(|o| o.0 == cur) {
            pdd.set_selected(i as u32);
        }
        pdd.set_valign(gtk4::Align::Center);
        pdd.connect_selected_notify(move |d| {
            if let Some((v, _)) = POS.get(d.selected() as usize) {
                let _ = panel.set_string("notification-position", v);
            }
        });
        card.append(&common::row(
            &gettext("Position"),
            &gettext("Which screen corner notifications slide into"),
            pdd.upcast(),
        ));
    }

    // Row: history retention — how long past notifications stay in
    // Raven's list (writes `history_days`; -1 keeps everything).
    const HISTORY_OPTS: &[(&str, &str)] = &[
        ("0", msgid("Immediately")),
        ("1", msgid("For 1 day")),
        ("3", msgid("For 3 days")),
        ("7", msgid("For 1 week")),
        ("14", msgid("For 2 weeks")),
        ("-1", msgid("Forever")),
    ];
    let labels: Vec<String> = HISTORY_OPTS.iter().map(|o| gettext(o.1)).collect();
    let hdd = gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    let hcur = get("history_days").unwrap_or_else(|| "-1".into());
    if let Some(i) = HISTORY_OPTS.iter().position(|o| o.0 == hcur) {
        hdd.set_selected(i as u32);
    } else {
        hdd.set_selected(5); // Forever — matches Raven's default
    }
    hdd.set_valign(gtk4::Align::Center);
    hdd.connect_selected_notify(|d| {
        if let Some((v, _)) = HISTORY_OPTS.get(d.selected() as usize) {
            write("history_days", v);
        }
    });
    card.append(&common::row(
        &gettext("Keep history"),
        &gettext("How long past notifications stay in Raven's list"),
        hdd.upcast(),
    ));
    page.append(&card);

    // ----------------------------------------------------------- //
    // Card 2: event sources — Raven watches these itself and       //
    // turns them into notifications. Live-toggled via ini watch.   //
    // ----------------------------------------------------------- //
    let events = common::section(&gettext("Events"));
    let ecard = common::card();

    // Row: removable media (mount/drive connect + disconnect).
    let msw = gtk4::Switch::new();
    msw.set_active(get("watch_mounts").as_deref() != Some("false"));
    msw.set_valign(gtk4::Align::Center);
    msw.connect_state_set(|_, on| {
        write("watch_mounts", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    ecard.append(&common::row(
        &gettext("Removable media"),
        &gettext("Notify when drives are connected, mounted or removed"),
        msw.upcast(),
    ));

    // Row: pending package upgrades.
    let usw = gtk4::Switch::new();
    usw.set_active(get("watch_updates").as_deref() != Some("false"));
    usw.set_valign(gtk4::Align::Center);
    usw.connect_state_set(|_, on| {
        write("watch_updates", if on { "true" } else { "false" });
        gtk4::glib::Propagation::Proceed
    });
    ecard.append(&common::row(
        &gettext(msgid("System updates")),
        &gettext("Check periodically for pending package upgrades"),
        usw.upcast(),
    ));

    // Row: update-check interval in minutes.
    let iv = common::spin(5.0, 1440.0, 5.0);
    iv.set_value(
        get("update_interval_min")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(60.0),
    );
    iv.set_valign(gtk4::Align::Center);
    iv.connect_value_changed(|s| write("update_interval_min", &format!("{:.0}", s.value())));
    ecard.append(&common::row(
        &gettext("Check every"),
        &gettext("minutes between update checks"),
        iv.upcast(),
    ));
    events.append(&ecard);
    page.append(&events);

    // ----------------------------------------------------------- //
    // Card 3: category colours — accent bar on the card's left     //
    // edge; empty key keeps Raven's stock look.                    //
    // ----------------------------------------------------------- //
    let colours = common::section(&gettext("Colours"));
    let ccard = common::card();
    // (ini key, title, subtitle)
    const CATS: &[(&str, &str, &str)] = &[
        (
            "color_default",
            msgid("Default"),
            msgid("Notifications without a special category"),
        ),
        (
            "color_mount",
            msgid("Drives & mounts"),
            msgid("Removable media connect, mount and removal"),
        ),
        (
            "color_update",
            msgid("System updates"),
            msgid("Pending package upgrades"),
        ),
        (
            "color_appointment",
            msgid("Appointments"),
            msgid("Calendar reminders fired by Raven"),
        ),
    ];
    for (key, title, sub) in CATS {
        ccard.append(&colour_row(
            &get(key),
            key,
            &gettext(*title),
            &gettext(*sub),
        ));
    }
    colours.append(&ccard);
    page.append(&colours);

    // ----------------------------------------------------------- //
    // Card 4: on-screen display — separate ini, own module.        //
    // ----------------------------------------------------------- //
    osd::append(&page);

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

/// One colour row: ColorDialogButton writes `color_<cat>` (any CSS
/// colour string — Raven re-parses it), the undo button clears the
/// key so the category falls back to Raven's stock look.
fn colour_row(
    current: &Option<String>,
    key: &'static str,
    title: &str,
    sub: &str,
) -> gtk4::ListBoxRow {
    let btn = gtk4::ColorDialogButton::new(Some(gtk4::ColorDialog::new()));
    if let Some(rgba) = current
        .as_deref()
        .and_then(|v| gtk4::gdk::RGBA::parse(v).ok())
    {
        btn.set_rgba(&rgba);
    }
    btn.set_valign(gtk4::Align::Center);
    btn.connect_rgba_notify(move |b| write(key, &b.rgba().to_string()));
    let reset = gtk4::Button::from_icon_name("edit-undo-symbolic");
    reset.set_tooltip_text(Some(&gettext("Reset to default colour")));
    reset.set_valign(gtk4::Align::Center);
    reset.connect_clicked(move |_| write(key, ""));
    let ctl = Box::new(Orientation::Horizontal, 6);
    ctl.append(&btn);
    ctl.append(&reset);
    common::row(title, sub, ctl.upcast())
}
