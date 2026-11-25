// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Updates" panel
// Copyright (C) 2026 LieraOS Project
//
// Beginner-facing update state: how many updates are waiting (from
// a non-privileged apt simulation), when lists were last refreshed,
// and one switch for automatic background installation restricted
// to the Debian archive. Privileged actions go through pkexec on
// worker threads — see auto.rs; the apt logic lives in apt.rs.

pub mod apt;
pub mod auto;
pub mod terminal;

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::cell::Cell;
use std::rc::Rc;

use super::common;

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Updates"));

    // -- Updates available ---------------------------------------------------
    let card = common::card();
    let status = gtk4::Label::new(Some(&gettext("Checking for updates…")));
    status.set_hexpand(true);
    status.set_xalign(0.0);
    status.set_valign(gtk4::Align::Center);
    let spinner = gtk4::Spinner::new();
    spinner.set_valign(gtk4::Align::Center);
    card.append(&plain_row(&status, spinner.clone().upcast()));

    let checked = gtk4::Label::new(Some(&gettext("Last checked: —")));
    checked.set_hexpand(true);
    checked.set_xalign(0.0);
    checked.set_valign(gtk4::Align::Center);
    card.append(&plain_row(&checked, gtk4::Label::new(None).upcast()));

    let check_btn = gtk4::Button::with_label(&gettext("Check for updates"));
    check_btn.set_valign(gtk4::Align::Center);
    let install_btn = gtk4::Button::with_label(&gettext("Install updates"));
    install_btn.set_valign(gtk4::Align::Center);
    install_btn.set_sensitive(false);
    install_btn.set_tooltip_text(Some(&gettext(
        "Opens a terminal window that shows the progress",
    )));
    let buttons = Box::new(Orientation::Horizontal, 8);
    buttons.set_hexpand(true);
    buttons.set_halign(gtk4::Align::End);
    buttons.append(&check_btn);
    buttons.append(&install_btn);
    card.append(&plain_row(&buttons, gtk4::Label::new(None).upcast()));
    page.append(&card);

    // Inline feedback line for async failures (hidden until needed).
    let err = gtk4::Label::new(None);
    err.add_css_class("error");
    err.add_css_class("caption");
    err.set_xalign(0.0);
    err.set_wrap(true);
    err.set_visible(false);
    page.append(&err);

    // -- Automatic updates ---------------------------------------------------
    let auto_sec = common::section(&gettext("Automatic updates"));
    let auto_card = common::card();
    populate_auto_card(&auto_card, &spinner, &err);
    auto_sec.append(&auto_card);
    page.append(&auto_sec);

    page.append(&common::note(&gettext(
        "Automatic updates only install packages from Debian. \
         Other sources on this computer are never touched.",
    )));

    // -- Wiring ---------------------------------------------------------------
    {
        // "Check for updates": privileged refresh, then recount.
        let (s, c, b, sp, e) = (
            status.clone(),
            checked.clone(),
            install_btn.clone(),
            spinner.clone(),
            err.clone(),
        );
        check_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            e.set_visible(false);
            sp.start();
            let (s, c, b, sp, e, btn) = (
                s.clone(),
                c.clone(),
                b.clone(),
                sp.clone(),
                e.clone(),
                btn.clone(),
            );
            auto::refresh_lists(std::boxed::Box::new(move |res| {
                btn.set_sensitive(true);
                match res {
                    Ok(()) => refresh_count(&s, &c, &b, &sp),
                    Err(m) => {
                        sp.stop();
                        e.set_label(&(gettext("Could not check for updates: ") + &m));
                        e.set_visible(true);
                    }
                }
            }));
        });
    }
    install_btn.connect_clicked(|_| {
        if let Some(term) = resolve_terminal() {
            // Open the user's terminal running update + dist-upgrade
            // under pkexec so progress stays visible. No -y: apt
            // prints its summary (incl. removals) and the user
            // confirms inside the terminal — matching what the
            // "N updates available" count (dist-upgrade) promises.
            let script = "pkexec sh -c \"apt-get update && apt-get dist-upgrade; \
                          printf '\\nPress Enter to close. '; read _\"";
            let argv: Vec<String> = terminal::argv(&term, script);
            let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            let _ = common::spawn_detached(&term, &refs);
        }
    });

    refresh_count(&status, &checked, &install_btn, &spinner);
    page.upcast()
}

/// A card row whose single child fills the width — used for the
/// status/buttons lines that don't fit the title+subtitle pattern.
fn plain_row(child: &impl IsA<gtk4::Widget>, right: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    inner.append(child);
    inner.append(&right);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Re-run the apt simulation off-thread and repaint status,
/// last-checked line and the Install button's sensitivity.
fn refresh_count(
    status: &gtk4::Label,
    checked: &gtk4::Label,
    install_btn: &gtk4::Button,
    spinner: &gtk4::Spinner,
) {
    status.set_label(&gettext("Checking for updates…"));
    spinner.start();
    let (s, c, b, sp) = (
        status.clone(),
        checked.clone(),
        install_btn.clone(),
        spinner.clone(),
    );
    auto::count_async(std::boxed::Box::new(move |res| {
        sp.stop();
        if let Ok(n) = res {
            s.set_label(&if n == 0 {
                gettext("Your system is up to date")
            } else {
                format!("{n} {}", gettext("updates available"))
            });
            b.set_sensitive(n > 0 && resolve_terminal().is_some());
        } else {
            s.set_label(&gettext("Could not check for updates"));
        }
        c.set_label(
            &apt::last_checked()
                .map(|t| gettext("Last checked") + ": " + &t)
                .unwrap_or_else(|| gettext("Last checked: never")),
        );
    }));
}

/// Fill the auto-updates card: the real switch when
/// unattended-upgrades is installed, otherwise a row that explains
/// itself and offers to install the package.
fn populate_auto_card(card: &ListBox, spinner: &gtk4::Spinner, err: &gtk4::Label) {
    while let Some(child) = card.first_child() {
        card.remove(&child);
    }
    if apt::unattended_upgrades_installed() {
        let sw = gtk4::Switch::new();
        sw.set_valign(gtk4::Align::Center);
        sw.set_active(apt::auto_enabled());
        // Reentrancy guard: the failure path below flips the switch
        // back with set_active, which emits ::state-set again — the
        // guard makes that second emission a no-op instead of
        // re-running apply_auto for the reverted value.
        let guard = Rc::new(Cell::new(false));
        let (sp, e, swc) = (spinner.clone(), err.clone(), sw.clone());
        sw.connect_state_set(move |_, on| {
            if guard.get() {
                return gtk4::glib::Propagation::Proceed;
            }
            sp.start();
            e.set_visible(false);
            let (sp, sw, e, g) = (sp.clone(), swc.clone(), e.clone(), Rc::clone(&guard));
            auto::apply_auto(
                on,
                std::boxed::Box::new(move |res| {
                    sp.stop();
                    if let Err(m) = res {
                        g.set(true);
                        sw.set_active(!on);
                        g.set(false);
                        e.set_label(&(gettext("Could not change automatic updates: ") + &m));
                        e.set_visible(true);
                    }
                }),
            );
            gtk4::glib::Propagation::Proceed
        });
        card.append(&common::row(
            &gettext("Install updates automatically"),
            &gettext(
                "Keeps your system up to date in the background. Only \
                 packages from Debian are installed — third-party \
                 sources such as Brave or VS Code are never touched \
                 automatically.",
            ),
            sw.upcast(),
        ));
    } else {
        // No dead switch: the row explains why the feature is
        // unavailable and offers the fix directly.
        let btn = gtk4::Button::with_label(&gettext("Install"));
        btn.set_valign(gtk4::Align::Center);
        let (card_c, sp, e) = (card.clone(), spinner.clone(), err.clone());
        btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            e.set_visible(false);
            sp.start();
            let (card_c, sp, e, b) = (card_c.clone(), sp.clone(), e.clone(), b.clone());
            auto::install_unattended_upgrades(std::boxed::Box::new(move |res| match res {
                Ok(()) => populate_auto_card(&card_c, &sp, &e),
                Err(m) => {
                    sp.stop();
                    b.set_sensitive(true);
                    e.set_label(&(gettext("Could not install the update service: ") + &m));
                    e.set_visible(true);
                }
            }));
        });
        card.append(&common::row(
            &gettext("Install updates automatically"),
            &gettext("Automatic updates need the 'unattended-upgrades' package."),
            btn.upcast(),
        ));
    }
}

/// The user's configured terminal (ragnar-view `terminal` key),
/// falling back to the first installed emulator ragnar-view
/// supports — needed so "Install updates" can show progress.
fn resolve_terminal() -> Option<String> {
    if let Some(s) = common::schema_lookup("org.lieraos.ragnar-view", "terminal") {
        let t = s.string("terminal").to_string();
        if !t.is_empty() && gtk4::glib::find_program_in_path(&t).is_some() {
            return Some(t);
        }
    }
    [
        "gnome-terminal",
        "kgx",
        "konsole",
        "xfce4-terminal",
        "alacritty",
        "kitty",
        "foot",
        "tilix",
        "terminator",
        "xterm",
    ]
    .iter()
    .find(|t| gtk4::glib::find_program_in_path(t).is_some())
    .map(|s| s.to_string())
}
