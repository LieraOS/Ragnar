// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings-gui — display module: mode-change revert dialog
// Copyright (C) 2026 LieraOS Project
//
// Windows-style safety net: after applying a new resolution/refresh,
// a modal dialog counts down 15 seconds. "Keep" closes it; "Revert"
// or timeout restores the previous mode via the daemon — so a wrong
// mode never leaves the user with a black screen.

use gettextrs::{gettext, ngettext};
use gtk4::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

use super::dbus_client;
use ragnar_shared::display::Mode;

/// Show the countdown dialog. `anchor` is any widget inside the
/// settings window; `prev` is the mode to restore on revert/timeout.
/// `on_revert` runs after the mode is restored — used to resync the
/// controls back to the reverted values.
pub fn show_mode_revert(anchor: &gtk4::Widget, output: &str, prev: Mode, on_revert: Rc<dyn Fn()>) {
    let window = gtk4::Window::new();
    window.set_modal(true);
    window.set_title(Some(&gettext("Keep these display settings?")));
    window.set_resizable(false);
    if let Some(parent) = anchor
        .root()
        .and_then(|r| r.downcast::<gtk4::Window>().ok())
    {
        window.set_transient_for(Some(&parent));
    }

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let countdown = gtk4::Label::new(Some(
        &ngettext("Reverting in {} second…", "Reverting in {} seconds…", 15)
            .replacen("{}", "15", 1),
    ));
    content.append(&countdown);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    let keep = gtk4::Button::with_label(&gettext("Keep"));
    keep.add_css_class("suggested-action");
    let revert = gtk4::Button::with_label(&gettext("Revert"));
    buttons.append(&keep);
    buttons.append(&revert);
    content.append(&buttons);

    window.set_child(Some(&content));

    let remaining = Rc::new(Cell::new(15u32));

    // Restore previous mode + apply via daemon.
    let do_revert = {
        let output = output.to_string();
        let prev = prev.clone();
        move || {
            tracing::info!(
                "ragnar-settings: reverting {} to {}x{}@{:.2}Hz",
                output,
                prev.width,
                prev.height,
                prev.refresh_rate as f64 / 1000.0
            );
            dbus_client::send_mode(&output, prev.width, prev.height, prev.refresh_rate);
            dbus_client::send_apply();
            on_revert();
        }
    };

    // 1-second countdown; timeout auto-reverts.
    {
        let countdown = countdown.clone();
        let window = window.clone();
        let remaining = remaining.clone();
        let do_revert = do_revert.clone();
        gtk4::glib::timeout_add_seconds_local(1, move || {
            if !window.is_visible() {
                return gtk4::glib::ControlFlow::Break;
            }
            let left = remaining.get();
            if left <= 1 {
                do_revert();
                window.close();
                return gtk4::glib::ControlFlow::Break;
            }
            remaining.set(left - 1);
            countdown.set_label(
                &ngettext(
                    "Reverting in {} second…",
                    "Reverting in {} seconds…",
                    left - 1,
                )
                .replacen("{}", &(left - 1).to_string(), 1),
            );
            gtk4::glib::ControlFlow::Continue
        });
    }

    keep.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    revert.connect_clicked({
        let window = window.clone();
        move |_| {
            do_revert();
            window.close();
        }
    });

    window.present();
}
