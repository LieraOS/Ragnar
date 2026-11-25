// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — login panel
//
// The widget stack under the artwork's ornament band: display
// name, password entry, session dropdown and the sign-in button —
// styled translucent over the nebula like GRUB's free-floating
// menu text. The panel exposes the raw widgets; greeter.rs owns
// the authentication wiring.

use gtk4::prelude::*;
use gtk4::{Box, Button, DropDown, Entry, Label, ListBox, MenuButton, Orientation,
           PasswordEntry, Popover};

use crate::i18n::tr;

/// Everything the auth wiring needs to reach.
pub struct LoginPanel {
    pub widget: Box,
    /// Free-text name entry — always editable; the menu button
    /// next to it fills it when the daemon lists known users.
    pub user_entry: Entry,
    /// User-list button (None when hide-users or empty list).
    pub user_menu: Option<MenuButton>,
    pub password: PasswordEntry,
    pub session_dd: DropDown,
    /// Session keys parallel to the dropdown items.
    pub session_keys: Vec<String>,
    pub button: Button,
    pub message: Label,
}

impl LoginPanel {
    /// Build the centred login box. `users` are (login, display)
    /// pairs — an empty list (or hide-users hint) switches to a
    /// free-text name entry instead of the dropdown.
    pub fn new(users: &[(String, String)], sessions: &[(String, String)],
               default_session: &str, hide_users: bool) -> Self {
        let widget = Box::new(Orientation::Vertical, 14);
        widget.add_css_class("login-panel");
        widget.set_width_request(380);
        widget.set_margin_top(24);
        widget.set_margin_bottom(24);
        widget.set_margin_start(30);
        widget.set_margin_end(30);

        // ---- user row -------------------------------------------------
        // Always an editable entry; when the daemon lists users a
        // menu button opens the list and picking a user fills it.
        let user_entry = Entry::new();
        user_entry.set_placeholder_text(Some(&tr("Benutzername")));
        user_entry.set_hexpand(true);

        let user_row = Box::new(Orientation::Horizontal, 6);
        user_row.append(&user_entry);

        let user_menu = if hide_users || users.is_empty() {
            None
        } else {
            let list_box = ListBox::new();
            list_box.add_css_class("user-list");
            for (_, display) in users {
                let label = Label::new(Some(display));
                label.set_xalign(0.0);
                label.set_margin_start(10);
                label.set_margin_end(10);
                label.set_margin_top(6);
                label.set_margin_bottom(6);
                list_box.append(&label);
            }
            let popover = Popover::new();
            popover.set_child(Some(&list_box));
            let menu_btn = MenuButton::new();
            menu_btn.set_icon_name("go-down-symbolic");
            menu_btn.set_tooltip_text(Some(&tr("Benutzer")));
            menu_btn.set_popover(Some(&popover));
            user_row.append(&menu_btn);

            // Picking a user writes the LOGIN name into the entry
            // (that's what authenticate() needs) and dismisses.
            let logins: Vec<String> = users.iter().map(|(l, _)| l.clone()).collect();
            let entry = user_entry.clone();
            let pop = popover.clone();
            list_box.connect_row_selected(move |_, row| {
                if let Some(r) = row {
                    if let Some(name) = logins.get(r.index() as usize) {
                        entry.set_text(name);
                        pop.popdown();
                    }
                }
            });
            Some(menu_btn)
        };
        widget.append(&user_row);

        // ---- password -------------------------------------------------
        let password = PasswordEntry::new();
        password.set_property("placeholder-text", tr("Passwort"));
        password.set_show_peek_icon(true);
        widget.append(&password);

        // Enter in the name field steps down to the password —
        // the natural type-Tab-type flow without leaving keys.
        let pw = password.clone();
        user_entry.connect_activate(move |_| {
            pw.grab_focus();
        });

        // ---- session picker -------------------------------------------
        let s_names: Vec<String> = sessions.iter().map(|(_, n)| n.clone()).collect();
        let session_keys: Vec<String> = sessions.iter().map(|(k, _)| k.clone()).collect();
        let session_dd = DropDown::from_strings(
            &s_names.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        session_dd.set_tooltip_text(Some(&tr("Sitzung")));
        if let Some(i) = session_keys.iter().position(|k| k == default_session) {
            session_dd.set_selected(i as u32);
        }
        if sessions.len() > 1 {
            widget.append(&session_dd);
        }

        // ---- sign-in + message -----------------------------------------
        let button = Button::with_label(&tr("Anmelden"));
        button.add_css_class("suggested-action");
        button.add_css_class("login-button");
        widget.append(&button);

        let message = Label::new(None);
        message.add_css_class("login-message");
        message.set_wrap(true);
        widget.append(&message);

        Self {
            widget,
            user_entry,
            user_menu,
            password,
            session_dd,
            session_keys,
            button,
            message,
        }
    }

    /// The login name to authenticate — typed directly or filled
    /// by the user-list button, the entry is always the source.
    pub fn username(&self) -> String {
        self.user_entry.text().to_string()
    }

    /// Session key for start_session_sync (empty → daemon default).
    pub fn session_key(&self) -> String {
        pick(&self.session_keys, self.session_dd.selected())
    }

    /// Grey the controls during an auth round-trip.
    pub fn set_busy(&self, busy: bool) {
        self.user_entry.set_sensitive(!busy);
        if let Some(m) = &self.user_menu {
            m.set_sensitive(!busy);
        }
        self.password.set_sensitive(!busy);
        self.session_dd.set_sensitive(!busy);
        self.button.set_sensitive(!busy);
    }

    /// Show a status/error line under the button.
    pub fn set_message(&self, text: &str, is_error: bool) {
        self.message.set_label(text);
        if is_error {
            self.message.add_css_class("error");
        } else {
            self.message.remove_css_class("error");
        }
    }
}

/// Element at `idx` or "" — dropdown selections can point past a
/// list that was empty at build time (e.g. single-session systems
/// where the picker is hidden).
fn pick(list: &[String], idx: u32) -> String {
    list.get(idx as usize).cloned().unwrap_or_default()
}

/// Power buttons (suspend/hibernate/restart/shutdown) for actions
/// the daemon advertises — returned as (button, action) pairs so
/// the caller wires the clicks.
pub fn power_buttons(can: (bool, bool, bool, bool)) -> Vec<(Button, &'static str)> {
    let (suspend, hibernate, restart, shutdown) = can;
    let mut out = Vec::new();
    for (show, icon, tip, action) in [
        // -symbolic is the modern standard name (Adwaita, hicolor
        // fallbacks); the plain names only exist in legacy themes.
        (suspend, "system-suspend-symbolic", tr("Bereitschaft"), "suspend"),
        (hibernate, "system-hibernate-symbolic", tr("Ruhezustand"), "hibernate"),
        (restart, "system-reboot-symbolic", tr("Neustart"), "restart"),
        (shutdown, "system-shutdown-symbolic", tr("Herunterfahren"), "shutdown"),
    ] {
        if !show {
            continue;
        }
        let b = Button::from_icon_name(icon);
        b.set_tooltip_text(Some(&tip));
        b.add_css_class("power-button");
        out.push((b, action));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::pick;

    #[test]
    fn pick_returns_element_or_empty() {
        let list = vec!["a".to_string(), "b".to_string()];
        assert_eq!(pick(&list, 1), "b");
        assert_eq!(pick(&list, 0), "a");
        // Past-the-end (hidden dropdown on single-item lists) → ""
        assert_eq!(pick(&list, 5), "");
        assert_eq!(pick(&[], 0), "");
    }
}
