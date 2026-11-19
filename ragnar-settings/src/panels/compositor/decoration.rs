// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor panel: window decoration
// Copyright (C) 2026 LieraOS Project
//
// The `<theme>` block: corner radius, drop shadows, the border
// kept on undecorated windows, the titlebar button layout and the
// titlebar font (the `<font place="ActiveWindow">` block).

use super::widgets::{attr_entry_row, choice_row, spin_row, switch_row};
use crate::i18n::msgid;
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;

/// Who draws titlebars — <core><decoration>. "server" offers
/// compositor-drawn decorations to every window.
const DECO_OPTS: &[(&str, &str)] = &[
    ("server", msgid("Compositor-drawn")),
    ("client", msgid("App-drawn")),
];

/// Titlebar button layout presets: (labwc layout, GNOME layout,
/// label). labwc wants word identifiers around exactly one colon —
/// a second colon or openbox letter-ids makes it silently render
/// its built-in default. The GNOME string is written to
/// `org.gnome.desktop.wm.preferences button-layout` so apps that
/// draw their own titlebar (GTK4) follow the same choice — that is
/// what makes the setting work on *every* window.
const LAYOUTS: &[(&str, &str, &str)] = &[
    (
        "icon:iconify,max,close",
        "icon:minimize,maximize,close",
        msgid("Icon left, minimize, maximize, close"),
    ),
    (
        "icon:iconify,close",
        "icon:minimize,close",
        msgid("Icon left, minimize, close"),
    ),
    (
        "menu,iconify,max,close:",
        "appmenu,minimize,maximize,close:",
        msgid("All buttons on the left"),
    ),
    (":close", ":close", msgid("Close only")),
];

/// The window-decoration card.
pub fn section() -> gtk4::Box {
    let b = common::section(&gettext("Window decoration"));
    let card = common::card();

    choice_row(
        &card,
        &gettext("Window titlebars"),
        &gettext("Whether the compositor or each app draws the titlebar."),
        "core",
        "decoration",
        DECO_OPTS,
        "server",
    );
    spin_row(
        &card,
        &gettext("Corner radius"),
        &gettext("Rounded window corners, in pixels. 0 draws sharp corners."),
        "theme",
        "cornerRadius",
        (0.0, 20.0),
        0.0,
    );
    switch_row(
        &card,
        &gettext("Drop shadows"),
        &gettext("Draw a soft shadow around windows."),
        "theme",
        "dropShadows",
        true,
    );
    switch_row(
        &card,
        &gettext("Keep window border"),
        &gettext("Show a thin border even on undecorated windows."),
        "theme",
        "keepBorder",
        true,
    );
    // Titlebar layout lives one level deeper: <theme><titlebar><layout>.
    // The generic choice helper targets <section><key> only, so this
    // row is wired by hand.
    build_layout_row(&card);
    // The titlebar font is the `name` attribute on the
    // attribute-keyed <font place="ActiveWindow" …/> element.
    attr_entry_row(
        &card,
        &gettext("Titlebar font"),
        &gettext("Font family used for window titles."),
        "font place=\"ActiveWindow\"",
        "name",
        "Sans",
    );

    b.append(&card);
    b.append(&common::note(&gettext(
        "These apply to windows the compositor decorates — X11 apps \
         and apps that ask for it. Apps that draw their own titlebar \
         (GTK4 and most native apps) are not affected.",
    )));
    b
}

/// Titlebar layout dropdown — writes `<titlebar><layout>` for
/// compositor-drawn titlebars and `button-layout` for app-drawn
/// ones, so the choice holds on every window.
fn build_layout_row(card: &gtk4::ListBox) {
    let cur = crate::panels::rcxml::get_in("<titlebar>", "</titlebar>", "layout")
        .unwrap_or_else(|| "icon:iconify,max,close".into());
    let sel = LAYOUTS
        .iter()
        .position(|(id, _, _)| *id == cur.trim())
        .unwrap_or(0) as u32;
    let labels: Vec<String> = LAYOUTS.iter().map(|(_, _, l)| gettext(*l)).collect();
    let dd = gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    dd.set_selected(sel);
    dd.set_valign(gtk4::Align::Center);
    dd.connect_selected_notify(move |d| {
        if let Some((rc, gnome, _)) = LAYOUTS.get(d.selected() as usize) {
            let (rc, gnome) = ((*rc).to_string(), (*gnome).to_string());
            crate::panels::rcxml::set_in("<titlebar>", "</titlebar>", "layout", &rc, Some("theme"));
            crate::panels::rcxml::reload();
            // App-drawn titlebars (GTK4 & friends) read the GNOME
            // key — same choice, other mechanism.
            if let Some(s) =
                common::schema_lookup("org.gnome.desktop.wm.preferences", "button-layout")
            {
                let _ = s.set_string("button-layout", &gnome);
            }
        }
    });
    card.append(&common::row(
        &gettext("Titlebar layout"),
        &gettext("Button order and sides in the window titlebar."),
        dd.upcast(),
    ));
}
