// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Privacy" panel
// Copyright (C) 2026 LieraOS Project
//
// Screen-lock behaviour via the org.gnome.desktop.screensaver /
// session gsettings keys — consumed live by ragnar-daemon's
// screenlock service, which rebuilds the swayidle command.
// Deliberately no Location section: GeoClue is not installed, a
// toggle would be a dead switch.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{gio, Box, ListBox, Orientation};

const SCREENS: &str = "org.gnome.desktop.screensaver";

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let heading = gtk4::Label::new(Some(&gettext("Privacy")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");

    // -- Master: screen locking on idle ------------------------------------
    let s = gio::Settings::new(SCREENS);
    let sw = gtk4::Switch::new();
    sw.set_active(s.boolean("idle-activation-enabled"));
    sw.set_valign(gtk4::Align::Center);
    sw.connect_state_set(|_, on| {
        let _ = gio::Settings::new(SCREENS).set_boolean("idle-activation-enabled", on);
        gtk4::glib::Propagation::Proceed
    });
    card.append(&row(
        &gettext("Screen lock"),
        &gettext(
            "Require a password when the session goes idle. \
                  Off: moving the mouse brings the screen back \
                  without unlocking.",
        ),
        sw.upcast(),
    ));

    // -- Lock delay after blank --------------------------------------------
    const OPTS: &[(i32, &str)] = &[
        (0, "Immediately"),
        (30, "30 seconds"),
        (60, "1 minute"),
        (300, "5 minutes"),
        (600, "10 minutes"),
    ];
    let cur = s.uint("lock-delay") as i32;
    let dd = gtk4::DropDown::from_strings(&OPTS.iter().map(|o| o.1).collect::<Vec<_>>());
    if let Some(i) = OPTS.iter().position(|o| o.0 == cur) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    dd.connect_selected_notify(move |d| {
        if let Some((v, _)) = OPTS.get(d.selected() as usize) {
            let _ = gio::Settings::new(SCREENS).set_uint("lock-delay", *v as u32);
        }
    });
    card.append(&row(
        &gettext("Lock after"),
        &gettext(
            "Delay between the screen going blank and the \
                  lock appearing",
        ),
        dd.upcast(),
    ));

    page.append(&card);

    let note = gtk4::Label::new(Some(&gettext(
        "Suspending also locks the screen (handled by the session \
         lock service). Location services are not installed on \
         this system.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    // -- Keyring: auto-unlock policy ----------------------------------------
    // ragnar-keyring reads ~/.config/ragnar-keyring/settings.ini at
    // session start; enabled (default) it feeds an empty master
    // password so the login keyring unlocks silently — matching the
    // behaviour users know from other desktops. Disabled, the normal
    // unlock prompt appears on first secret access.
    let card2 = ListBox::new();
    card2.set_selection_mode(gtk4::SelectionMode::None);
    card2.add_css_class("boxed-list");
    let ksw = gtk4::Switch::new();
    ksw.set_active(keyring_auto_unlock());
    ksw.set_valign(gtk4::Align::Center);
    ksw.connect_state_set(|_, on| {
        write_keyring_policy(on);
        gtk4::glib::Propagation::Proceed
    });
    card2.append(&row(
        &gettext("Automatic keyring unlock"),
        &gettext(
            "Unlock the login keyring without a password prompt                   (required for autologin). The keyring then stores                   secrets unencrypted — off: enter your password once                   per session instead.",
        ),
        ksw.upcast(),
    ));
    page.append(&card2);

    page.upcast()
}

/// Path of the keyring policy file shared with ragnar-keyring.
fn keyring_settings_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    std::path::PathBuf::from(home).join(".config/ragnar-keyring/settings.ini")
}

/// Reads the auto-unlock flag; defaults to true (no prompt).
fn keyring_auto_unlock() -> bool {
    match std::fs::read_to_string(keyring_settings_path()) {
        Ok(text) => !text.lines().any(|l| l.trim() == "auto-unlock=false"),
        Err(_) => true,
    }
}

/// Persists the toggle for the next session start.
fn write_keyring_policy(on: bool) {
    let path = keyring_settings_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(
        path,
        format!("[keyring]\nauto-unlock={}\n", if on { "true" } else { "false" }),
    );
}

/// Card row: title + subtitle left, control right.
fn row(title: &str, sub: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
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
    let s = gtk4::Label::new(Some(sub));
    s.add_css_class("dim-label");
    s.add_css_class("caption");
    s.set_xalign(0.0);
    s.set_wrap(true);
    texts.append(&s);
    inner.append(&texts);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}
