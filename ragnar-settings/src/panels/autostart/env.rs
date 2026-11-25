// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — session environment editor
// Copyright (C) 2026 LieraOS Project
//
// KEY=VALUE editor for the session's environment file. Comments
// and blank lines are preserved verbatim; values edit in place
// (commit on Enter or focus-leave); new pairs append at the end.
// Variables the desktop session manages itself (keyboard layout,
// cursor, locale — see the file's own header) render read-only:
// editing them here would silently lose to the session's next
// write, like the managed keybinds.

use super::{read_lines, write_lines};
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::Box;
use std::cell::RefCell;
use std::rc::Rc;

/// Variable prefixes/names the session owns — matches the managed
/// list documented in the environment file's header.
fn is_managed(key: &str) -> bool {
    key.starts_with("XKB_")
        || key.starts_with("XCURSOR_")
        || key.starts_with("LC_")
        || key == "LANG"
}

/// Parsed KEY=VALUE entries: (line index, key, value, managed).
fn vars(lines: &[String]) -> Vec<(usize, String, String, bool)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                return None;
            }
            let (k, v) = t.split_once('=')?;
            Some((i, k.to_string(), v.to_string(), is_managed(k)))
        })
        .collect()
}

/// Re-render the variables card from the file on disk.
fn render(card: &gtk4::ListBox, refresh: Rc<dyn Fn()>) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    let lines = read_lines("environment");
    for (idx, key, val, managed) in vars(&lines) {
        if managed {
            let lock = gtk4::Image::from_icon_name("system-lock-screen-symbolic");
            lock.set_tooltip_text(Some(&gettext("Managed by the desktop session")));
            card.append(&common::row(&key, &val, lock.upcast()));
            continue;
        }
        let e = gtk4::Entry::builder()
            .text(&val)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        {
            let save = move |e: &gtk4::Entry| {
                let mut ls = read_lines("environment");
                if idx < ls.len() {
                    let k = ls[idx].split('=').next().unwrap_or("").to_string();
                    ls[idx] = format!("{k}={}", e.text());
                    write_lines("environment", &ls);
                }
            };
            let s = save;
            e.connect_activate(move |e| s(e));
            let focus = gtk4::EventControllerFocus::new();
            focus.connect_leave(move |e| {
                if let Some(e) = e.widget().and_downcast::<gtk4::Entry>() {
                    save(&e)
                }
            });
            e.add_controller(focus);
        }
        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        del.set_tooltip_text(Some(&gettext("Remove variable")));
        del.set_valign(gtk4::Align::Center);
        {
            let refresh = refresh.clone();
            del.connect_clicked(move |_| {
                let mut ls = read_lines("environment");
                if idx < ls.len() {
                    ls.remove(idx);
                    write_lines("environment", &ls);
                }
                refresh();
            });
        }
        let row_box = Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.set_valign(gtk4::Align::Center);
        row_box.append(&e);
        row_box.append(&del);
        card.append(&common::row(&key, "", row_box.upcast()));
    }
}

/// Build the environment card: variable rows plus an add-field.
pub fn card() -> gtk4::Widget {
    let card = common::card();
    let cell = Rc::new(RefCell::new(None::<Rc<dyn Fn()>>));
    let refresh: Rc<dyn Fn()> = {
        let card = card.clone();
        let cell2 = cell.clone();
        let f = Rc::new(move || {
            let r = cell2.borrow().as_ref().unwrap().clone();
            render(&card, r);
        });
        *cell.borrow_mut() = Some(f.clone());
        f
    };
    render(&card, refresh.clone());

    let key_e = gtk4::Entry::builder()
        .placeholder_text(gettext("NAME"))
        .valign(gtk4::Align::Center)
        .build();
    let val_e = gtk4::Entry::builder()
        .placeholder_text(gettext("value"))
        .hexpand(true)
        .valign(gtk4::Align::Center)
        .build();
    let add_b = gtk4::Button::from_icon_name("list-add-symbolic");
    add_b.set_tooltip_text(Some(&gettext("Add variable")));
    add_b.set_valign(gtk4::Align::Center);
    let row_box = Box::new(gtk4::Orientation::Horizontal, 8);
    row_box.set_valign(gtk4::Align::Center);
    row_box.append(&key_e);
    row_box.append(&val_e);
    row_box.append(&add_b);
    card.append(&common::row(&gettext("New variable"), "", row_box.upcast()));
    {
        let (refresh, ke, ve) = (refresh.clone(), key_e.clone(), val_e.clone());
        let commit = move || {
            let (k, v) = (ke.text().trim().to_string(), ve.text().to_string());
            // A name must exist and must not collide with a managed
            // variable — those are owned by the session.
            if !k.is_empty() && !k.contains('=') && !is_managed(&k) {
                let mut ls = read_lines("environment");
                ls.push(format!("{k}={v}"));
                write_lines("environment", &ls);
                ke.set_text("");
                ve.set_text("");
                refresh();
            }
        };
        let c = commit.clone();
        add_b.connect_clicked(move |_| c());
        let c2 = commit.clone();
        val_e.connect_activate(move |_| c2());
        key_e.connect_activate(move |_| commit());
    }
    card.upcast()
}
