// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — autostart programs editor
// Copyright (C) 2026 LieraOS Project
//
// Editor for the session's autostart file: one command per line,
// comments and blank lines preserved verbatim. Entries are editable
// in place (commit on Enter or focus-leave), deletable, and new
// lines append at the end. The file is created on first save —
// labwc only reads it when present. Changes take effect at the
// next login, not retroactively.

use super::{read_lines, write_lines};
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::Box;
use std::cell::RefCell;
use std::rc::Rc;

/// Line indices that carry a command (not blank, not a comment).
fn command_lines(lines: &[String]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|(i, _)| i)
        .collect()
}

/// Re-render the list card from the file on disk.
fn render(card: &gtk4::ListBox, refresh: Rc<dyn Fn()>) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    let lines = read_lines("autostart");
    let cmds = command_lines(&lines);
    if cmds.is_empty() {
        card.append(&common::note(&gettext("No autostart entries yet.")));
    }
    for idx in cmds {
        let e = gtk4::Entry::builder()
            .text(&lines[idx])
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        {
            // Edit in place: commit replaces this exact line.
            let save = move |e: &gtk4::Entry| {
                let mut ls = read_lines("autostart");
                if idx < ls.len() {
                    ls[idx] = e.text().to_string();
                    write_lines("autostart", &ls);
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
        del.set_tooltip_text(Some(&gettext("Remove entry")));
        del.set_valign(gtk4::Align::Center);
        {
            let refresh = refresh.clone();
            del.connect_clicked(move |_| {
                let mut ls = read_lines("autostart");
                if idx < ls.len() {
                    ls.remove(idx);
                    write_lines("autostart", &ls);
                }
                refresh();
            });
        }
        let row_box = Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.set_valign(gtk4::Align::Center);
        row_box.append(&e);
        row_box.append(&del);
        card.append(&common::row("", "", row_box.upcast()));
    }
}

/// Build the autostart card: command rows plus an add-field.
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

    // Add-row: one entry plus a button appending the line.
    let add_e = gtk4::Entry::builder()
        .placeholder_text(gettext("Command to run at login, e.g. firefox"))
        .hexpand(true)
        .valign(gtk4::Align::Center)
        .build();
    let add_b = gtk4::Button::from_icon_name("list-add-symbolic");
    add_b.set_tooltip_text(Some(&gettext("Add entry")));
    add_b.set_valign(gtk4::Align::Center);
    let row_box = Box::new(gtk4::Orientation::Horizontal, 8);
    row_box.set_valign(gtk4::Align::Center);
    row_box.append(&add_e);
    row_box.append(&add_b);
    card.append(&common::row(&gettext("New entry"), "", row_box.upcast()));
    {
        let (refresh, ae) = (refresh.clone(), add_e.clone());
        let commit = move || {
            let cmd = ae.text().trim().to_string();
            if !cmd.is_empty() {
                let mut ls = read_lines("autostart");
                ls.push(cmd);
                write_lines("autostart", &ls);
                ae.set_text("");
                refresh();
            }
        };
        let c = commit.clone();
        add_b.connect_clicked(move |_| c());
        add_e.connect_activate(move |_| commit());
    }
    card.upcast()
}
