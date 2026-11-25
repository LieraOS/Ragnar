// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Accessibility" panel
// Copyright (C) 2026 LieraOS Project
//
// Beginner-facing accessibility options backed by standard GNOME
// gsettings that GTK4/libadwaita consume live (verified on this
// system): text-scaling-factor changes gtk-xft-dpi, enable-animations
// maps to GtkSettings gtk-enable-animations, and high-contrast flips
// the xdg-desktop-portal appearance contrast flag — which
// Adw.StyleManager.high_contrast follows (verified in a fresh
// process). Deliberately absent: reduced-motion (no GTK consumer —
// a dead switch) and cursor size (lives in Appearance → Cursor).
// The on-screen keyboard / screen reader toggles live in an
// "Experimental" section: they write the standard
// org.gnome.desktop.a11y.applications flags (real schema keys),
// but nothing consumes them unless an OSK (squeekboard etc.) or
// a screen reader (orca) is installed — each row reports that
// honestly instead of pretending the feature works.

use gettextrs::gettext;
use gtk4::prelude::*;

use super::common;

const INTERFACE: &str = "org.gnome.desktop.interface";
const A11Y: &str = "org.gnome.desktop.a11y.interface";
const A11Y_APPS: &str = "org.gnome.desktop.a11y.applications";

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Accessibility"));

    // -- Text --------------------------------------------------------------
    let text_sec = common::section(&gettext("Text"));
    let tc = common::card();
    tc.append(&text_size_row());
    text_sec.append(&tc);
    page.append(&text_sec);

    // -- Display -------------------------------------------------------------
    let disp_sec = common::section(&gettext("Display"));
    let dc = common::card();
    dc.append(&animations_row());
    dc.append(&contrast_row());
    disp_sec.append(&dc);
    page.append(&disp_sec);

    // -- Experimental ------------------------------------------------------
    // Real gsettings flags, but only useful once a consumer is
    // installed — the rows report whether that is the case.
    let exp_sec = common::section(&gettext("Experimental"));
    let ec = common::card();
    ec.append(&a11y_flag_row(
        "screen-keyboard-enabled",
        &gettext("On-screen keyboard"),
        &gettext("Virtual keyboard for touchscreens."),
        &["squeekboard", "wvkbd", "onboard"],
    ));
    ec.append(&a11y_flag_row(
        "screen-reader-enabled",
        &gettext("Screen reader"),
        &gettext("Reads screen contents aloud."),
        &["orca"],
    ));
    // Make the "not installed" hint actionable: offer a direct jump
    // into the software center when a consumer is missing and the
    // software app itself is available.
    let tools_missing =
        installed(&["squeekboard", "wvkbd", "onboard"]).is_none() || installed(&["orca"]).is_none();
    if tools_missing && installed(&["ragnar-software"]).is_some() {
        let btn = gtk4::Button::with_label(&gettext("Open Software"));
        btn.set_valign(gtk4::Align::Center);
        btn.connect_clicked(|_| {
            let _ = common::spawn_detached("ragnar-software", &[]);
        });
        ec.append(&common::row(
            &gettext("Install tools"),
            &gettext("On-screen keyboards and screen readers can be installed via Software."),
            btn.upcast(),
        ));
    }
    exp_sec.append(&ec);
    page.append(&exp_sec);

    page.append(&common::note(&gettext(
        "Some apps need to be restarted before changes apply \
         everywhere.",
    )));
    page.upcast()
}

/// "Text size" dropdown writing text-scaling-factor (type `d` —
/// a wrong type would fail silently and revert).
fn text_size_row() -> gtk4::ListBoxRow {
    const OPTS: &[f64] = &[1.0, 1.25, 1.5, 2.0];
    let labels = [
        gettext("Default (100%)"),
        gettext("Large (125%)"),
        gettext("Larger (150%)"),
        gettext("Largest (200%)"),
    ];
    let dd = gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    dd.set_valign(gtk4::Align::Center);
    match common::schema_lookup(INTERFACE, "text-scaling-factor") {
        Some(s) => {
            let cur = s.double("text-scaling-factor");
            // Nearest option wins — hand-set values like 1.1 still
            // preselect something sensible.
            let idx = OPTS
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (*a - cur).abs().partial_cmp(&(*b - cur).abs()).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
            dd.set_selected(idx as u32);
            dd.connect_selected_notify(move |d| {
                if let Some(v) = OPTS.get(d.selected() as usize) {
                    let _ = s.set_double("text-scaling-factor", *v);
                }
            });
            common::row(
                &gettext("Text size"),
                &gettext("Makes text larger in apps. Some apps need to be restarted."),
                dd.upcast(),
            )
        }
        None => missing_row(
            &gettext("Text size"),
            &gettext("Unavailable: the desktop settings service is not installed."),
            dd.upcast(),
        ),
    }
}

/// "Reduce animations" — inverted enable-animations so the beginner
/// wording reads naturally.
fn animations_row() -> gtk4::ListBoxRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    match common::schema_lookup(INTERFACE, "enable-animations") {
        Some(s) => {
            sw.set_active(!s.boolean("enable-animations"));
            sw.connect_state_set(move |_, on| {
                let _ = s.set_boolean("enable-animations", !on);
                gtk4::glib::Propagation::Proceed
            });
            common::row(
                &gettext("Reduce animations"),
                &gettext("Turns off window and menu animations."),
                sw.upcast(),
            )
        }
        None => missing_row(
            &gettext("Reduce animations"),
            &gettext("Unavailable: the desktop settings service is not installed."),
            sw.upcast(),
        ),
    }
}

/// "High contrast" — org.gnome.desktop.a11y.interface high-contrast;
/// Adw.StyleManager.high_contrast follows it live (verified).
fn contrast_row() -> gtk4::ListBoxRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    match common::schema_lookup(A11Y, "high-contrast") {
        Some(s) => {
            sw.set_active(s.boolean("high-contrast"));
            sw.connect_state_set(move |_, on| {
                let _ = s.set_boolean("high-contrast", on);
                gtk4::glib::Propagation::Proceed
            });
            common::row(
                &gettext("High contrast"),
                &gettext("Stronger colours and outlines for better readability."),
                sw.upcast(),
            )
        }
        None => missing_row(
            &gettext("High contrast"),
            &gettext("Unavailable: the accessibility settings service is not installed."),
            sw.upcast(),
        ),
    }
}

/// Experimental a11y-application flag (screen-keyboard-enabled /
/// screen-reader-enabled). Writes the real schema boolean and
/// extends the subtitle with whether a consumer program exists —
/// otherwise the switch would look like it does something it does
/// not yet do.
fn a11y_flag_row(
    key: &'static str,
    title: &str,
    subtitle: &str,
    consumers: &[&'static str],
) -> gtk4::ListBoxRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    let sub = match installed(consumers) {
        Some(bin) => format!("{subtitle} {}", bin),
        None => format!(
            "{subtitle} {}",
            gettext("Not installed — can be installed via Software.")
        ),
    };
    match common::schema_lookup(A11Y_APPS, key) {
        Some(s) => {
            sw.set_active(s.boolean(key));
            sw.connect_state_set(move |_, on| {
                let _ = s.set_boolean(key, on);
                gtk4::glib::Propagation::Proceed
            });
            common::row(title, &sub, sw.upcast())
        }
        None => missing_row(
            title,
            &gettext("Unavailable: the accessibility settings service is not installed."),
            sw.upcast(),
        ),
    }
}

/// First executable found on PATH among `names` — used to report
/// whether anything would consume the a11y flag.
fn installed(names: &[&'static str]) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        for n in names {
            let p = dir.join(n);
            if p.is_file()
                && p.metadata()
                    .map(|m| m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            {
                return Some(n.to_string());
            }
        }
    }
    None
}

/// Row rendered insensitive when its backing schema is absent — the
/// subtitle says why, the control is inert but never crashes.
fn missing_row(title: &str, why: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let r = common::row(title, why, control);
    r.set_sensitive(false);
    r
}
