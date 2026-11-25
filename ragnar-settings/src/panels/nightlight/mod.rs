// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Night light" panel
// Copyright (C) 2026 LieraOS Project
//
// Beginner-facing colour-temperature settings written to the
// standard org.gnome.settings-daemon.plugins.color keys — consumed
// by the standalone ragnar-nightlight service, which owns
// gammastep. The panel is pure UI: no config files, no process
// management. "All the time" is expressed as from == to == 0.0,
// which the service maps to equal day/night temperatures. There is
// deliberately no "sunset to sunrise" mode: no GeoClue service is
// installed, so the panel offers explicit times and permanent mode.

use gettextrs::gettext;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

use super::common;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.color";

/// Warmth slider bounds in Kelvin — never shown to the user.
const TEMP_MIN: f64 = 3000.0;
const TEMP_MAX: f64 = 6000.0;

/// How long after the last control change before the key is
/// written — dragging the slider must not rewrite settings on
/// every pixel.
const DEBOUNCE_MS: u32 = 300;

/// The 48 half-hour clock times offered by the From/Until dropdowns.
fn times() -> Vec<String> {
    (0..48)
        .map(|i| format!("{:02}:{:02}", i / 2, (i % 2) * 30))
        .collect()
}

/// "HH:MM" → fractional hours (20:30 → 20.5), the gsd key format.
fn hhmm_to_hours(hhmm: &str) -> f64 {
    let (h, m) = hhmm.split_once(':').unwrap_or(("0", "0"));
    h.parse::<f64>().unwrap_or(0.0) + m.parse::<f64>().unwrap_or(0.0) / 60.0
}

/// Fractional hours → dropdown index, nearest half hour.
fn hours_to_idx(hours: f64, times: &[String]) -> u32 {
    let h = hours as u32;
    let m = ((hours - h as f64) * 60.0).round() as u32;
    (h * 2 + (m >= 30) as u32).min(times.len() as u32 - 1)
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Night light"));
    let card = common::card();
    let Some(settings) = settings() else {
        page.append(&common::note(&gettext(
            "Night light settings are unavailable on this system.",
        )));
        return page.upcast();
    };
    let debounce: Rc<RefCell<Option<gtk4::glib::SourceId>>> = Rc::new(RefCell::new(None));
    // Pending key writes — each control queues its value; one timer
    // flushes them together so the service sees one change set.
    let pending: Rc<RefCell<Vec<(String, gtk4::glib::Variant)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let apply = {
        let settings = settings.clone();
        let debounce = Rc::clone(&debounce);
        let pending = Rc::clone(&pending);
        Rc::new(move || {
            if let Some(id) = debounce.borrow_mut().take() {
                id.remove();
            }
            let (settings, cell, pend) =
                (settings.clone(), Rc::clone(&debounce), Rc::clone(&pending));
            let id = gtk4::glib::timeout_add_local_once(
                std::time::Duration::from_millis(DEBOUNCE_MS as u64),
                move || {
                    cell.borrow_mut().take();
                    for (k, v) in pend.borrow_mut().drain(..) {
                        let _ = settings.set_value(&k, &v);
                    }
                },
            );
            *debounce.borrow_mut() = Some(id);
        })
    };
    /// Debounced writer callback signature: (gsettings key, value).
    type QueueFn = dyn Fn(&str, gtk4::glib::Variant);
    let queue: Rc<QueueFn> = {
        let pending = Rc::clone(&pending);
        Rc::new(move |key: &str, v: gtk4::glib::Variant| {
            pending.borrow_mut().push((key.to_string(), v));
        })
    };

    // -- Master switch --------------------------------------------------
    let available = gammastep_available();
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    sw.set_active(available && settings.boolean("night-light-enabled"));
    let master = common::row(
        &gettext("Night light"),
        &if available {
            gettext("Makes the screen warmer in the evening to reduce eye strain")
        } else {
            gettext("Night light needs the gammastep package.")
        },
        sw.clone().upcast(),
    );
    if !available {
        master.set_sensitive(false);
        card.append(&master);
        page.append(&card);
        return page.upcast();
    }
    card.append(&master);

    // -- Rows that only make sense while enabled ------------------------
    let mut option_rows: Vec<gtk4::ListBoxRow> = Vec::new();
    let times = times();
    let from_hours = settings.double("night-light-schedule-from");
    let to_hours = settings.double("night-light-schedule-to");
    let always = from_hours == 0.0 && to_hours == 0.0;

    let sched_labels = [gettext("Every evening"), gettext("All the time")];
    let sched =
        gtk4::DropDown::from_strings(&sched_labels.iter().map(String::as_str).collect::<Vec<_>>());
    sched.set_selected(always as u32);
    sched.set_valign(gtk4::Align::Center);
    option_rows.push(common::row(
        &gettext("Schedule"),
        &gettext("When the warmer colour should be active"),
        sched.clone().upcast(),
    ));

    let from = gtk4::DropDown::from_strings(&times.iter().map(String::as_str).collect::<Vec<_>>());
    from.set_selected(if always {
        40
    } else {
        hours_to_idx(from_hours, &times)
    });
    from.set_valign(gtk4::Align::Center);
    let from_row = common::row(
        &gettext("From"),
        &gettext("Time the screen starts getting warmer"),
        from.clone().upcast(),
    );
    option_rows.push(from_row.clone());

    let until = gtk4::DropDown::from_strings(&times.iter().map(String::as_str).collect::<Vec<_>>());
    until.set_selected(if always {
        12
    } else {
        hours_to_idx(to_hours, &times)
    });
    until.set_valign(gtk4::Align::Center);
    let until_row = common::row(
        &gettext("Until"),
        &gettext("Time the screen returns to normal"),
        until.clone().upcast(),
    );
    option_rows.push(until_row.clone());

    // Warmth slider: Kelvin internally, inverted so Warmer is right.
    let warm = gtk4::Scale::with_range(Orientation::Horizontal, TEMP_MIN, TEMP_MAX, 100.0);
    warm.set_value(settings.uint("night-light-temperature") as f64);
    warm.set_draw_value(false);
    warm.set_size_request(160, -1);
    warm.set_valign(gtk4::Align::Center);
    warm.set_inverted(true);
    let warm_box = Box::new(Orientation::Horizontal, 6);
    for (txt, pos) in [(gettext("Cooler"), 0), (gettext("Warmer"), 2)] {
        let l = gtk4::Label::new(Some(&txt));
        l.add_css_class("dim-label");
        l.add_css_class("caption");
        if pos == 0 {
            warm_box.append(&l);
            warm_box.append(&warm);
        } else {
            warm_box.append(&l);
        }
    }
    option_rows.push(common::row(
        &gettext("Warmth"),
        &gettext("How warm the screen colour gets"),
        warm_box.upcast(),
    ));

    for r in &option_rows {
        card.append(r);
        r.set_sensitive(sw.is_active());
    }
    let sync_time_rows = {
        let (fr, ur) = (from_row.clone(), until_row.clone());
        move |on: bool, always: bool| {
            fr.set_sensitive(on && !always);
            ur.set_sensitive(on && !always);
        }
    };
    sync_time_rows(sw.is_active(), always);

    // -- Signal wiring ------------------------------------------------------
    {
        let (opt, sync, st) = (
            option_rows.clone(),
            sync_time_rows.clone(),
            settings.clone(),
        );
        sw.connect_state_set(move |_, on| {
            let _ = st.set_boolean("night-light-enabled", on);
            for r in &opt {
                r.set_sensitive(on);
            }
            sync(on, false);
            gtk4::glib::Propagation::Proceed
        });
    }
    {
        let (q, ap, sync, on_ref) = (
            Rc::clone(&queue),
            Rc::clone(&apply),
            sync_time_rows.clone(),
            sw.clone(),
        );
        sched.connect_selected_notify(move |d| {
            let always = d.selected() == 1;
            if always {
                // Equal from/to marks "all the time" for the service.
                (*q)("night-light-schedule-from", 0.0f64.to_variant());
                (*q)("night-light-schedule-to", 0.0f64.to_variant());
            } else {
                (*q)("night-light-schedule-from", 20.0f64.to_variant());
                (*q)("night-light-schedule-to", 6.0f64.to_variant());
            }
            sync(on_ref.is_active(), always);
            ap();
        });
    }
    {
        let (q, ap, t) = (Rc::clone(&queue), Rc::clone(&apply), times.clone());
        from.connect_selected_notify(move |d| {
            if let Some(v) = t.get(d.selected() as usize) {
                (*q)("night-light-schedule-from", hhmm_to_hours(v).to_variant());
                ap();
            }
        });
    }
    {
        let (q, ap, t) = (Rc::clone(&queue), Rc::clone(&apply), times.clone());
        until.connect_selected_notify(move |d| {
            if let Some(v) = t.get(d.selected() as usize) {
                (*q)("night-light-schedule-to", hhmm_to_hours(v).to_variant());
                ap();
            }
        });
    }
    {
        let (q, ap) = (Rc::clone(&queue), Rc::clone(&apply));
        warm.connect_value_changed(move |s| {
            (*q)("night-light-temperature", (s.value() as u32).to_variant());
            ap();
        });
    }

    page.append(&card);
    page.append(&common::note(&gettext(
        "Changes apply a moment after you stop adjusting. Without a \
         location service the panel offers fixed times instead of \
         sunset tracking.",
    )));
    page.upcast()
}

/// The color schema, or None when missing.
fn settings() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source
        .lookup(SCHEMA, true)
        .map(|_| gio::Settings::new(SCHEMA))
}

/// Whether gammastep is installed — the row stays a hint only.
fn gammastep_available() -> bool {
    gtk4::glib::find_program_in_path("gammastep").is_some()
}
