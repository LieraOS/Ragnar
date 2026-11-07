// SPDX-License-Identifier: GPL-3.0-or-later
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
// a dead switch), on-screen keyboard and screen reader (nothing
// installed), and cursor size (lives in Appearance → Cursor).

use gettextrs::gettext;
use gtk4::prelude::*;

use super::common;

const INTERFACE: &str = "org.gnome.desktop.interface";
const A11Y: &str = "org.gnome.desktop.a11y.interface";

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

/// Row rendered insensitive when its backing schema is absent — the
/// subtitle says why, the control is inert but never crashes.
fn missing_row(title: &str, why: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let r = common::row(title, why, control);
    r.set_sensitive(false);
    r
}
