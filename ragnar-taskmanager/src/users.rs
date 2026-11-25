// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Users tab — logged-in users from logind's standard D-Bus API
//! (org.freedesktop.login1.Manager). Each row shows the user, their
//! session count and live process stats; activating a row jumps to
//! the Processes tab pre-filtered to that user — same flow the
//! original used between its Users and Processes tabs.

use std::rc::Rc;

use gettextrs::gettext as tr;
use gtk4::prelude::*;
use zbus::blocking::Connection;

use crate::procfs::ProcessTable;

/// Build the users page. `filter` selects a user in the Processes
/// tab and `stack` is switched to it on row activation.
pub fn page(filter: Rc<dyn Fn(Option<String>)>, stack: gtk4::Stack) -> gtk4::Widget {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);
    let hint = gtk4::Label::new(Some(&tr(
        "Double-click a user to show their processes.",
    )));
    hint.set_halign(gtk4::Align::Start);
    hint.add_css_class("dim-label");
    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .build();
    root.append(&hint);
    root.append(&scroll);

    let table = std::rc::Rc::new(std::cell::RefCell::new(ProcessTable::new()));

    list.connect_row_activated(move |_, row| {
        let uid = row.widget_name().to_string();
        filter(if uid.is_empty() { None } else { Some(uid) });
        stack.set_visible_child_name("processes");
    });

    let rebuild = {
        let list = list.clone();
        let table = table.clone();
        move || {
            table.borrow_mut().refresh();
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            for (uid, name) in logind_users() {
                let procs: Vec<_> = table
                    .borrow()
                    .rows
                    .iter()
                    .filter(|p| p.user == name)
                    .cloned()
                    .collect();
                let cpu: f32 = procs.iter().map(|p| p.cpu).sum();
                let mem: u64 = procs.iter().map(|p| p.rss_kb).sum();
                list.append(&row(&name, uid, sessions_of(&name),
                               procs.len(), cpu, mem));
            }
        }
    };

    rebuild();
    glib::timeout_add_local(crate::interval(), move || {
        if !list.is_visible() {
            return glib::ControlFlow::Continue;
        }
        rebuild();
        glib::ControlFlow::Continue
    });
    root.upcast()
}

/// One user row; widget_name carries the username for the filter jump.
fn row(name: &str, _uid: u32, sessions: usize, procs: usize,
       cpu: f32, mem_kb: u64) -> gtk4::ListBoxRow {
    let r = gtk4::ListBoxRow::new();
    r.set_widget_name(name);
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    b.set_margin_top(8);
    b.set_margin_bottom(8);
    b.set_margin_start(12);
    b.set_margin_end(12);
    let n = gtk4::Label::builder().label(name).halign(gtk4::Align::Start).build();
    n.add_css_class("heading");
    let s = gtk4::Label::new(Some(&format!("{}: {}", tr("Sessions"), sessions)));
    s.add_css_class("dim-label");
    let left = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    left.set_hexpand(true);
    left.append(&n);
    left.append(&s);
    let right = gtk4::Label::new(Some(&format!(
        "{}: {} | CPU: {:.1} % | {}: {}",
        tr("Processes"), procs, cpu, tr("Memory"),
        crate::util::kb_human(mem_kb),
    )));
    b.append(&left);
    b.append(&right);
    r.set_child(Some(&b));
    r
}

/// logind ListUsers → (uid, name) pairs.
fn logind_users() -> Vec<(u32, String)> {
    Connection::system()
        .and_then(|c| {
            c.call_method(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                Some("org.freedesktop.login1.Manager"),
                "ListUsers",
                &(),
            )
        })
        .and_then(|m| {
            m.body()
                .deserialize::<Vec<(u32, String, zbus::zvariant::OwnedObjectPath)>>()
                .map_err(zbus::Error::from)
        })
        .map(|v| v.into_iter().map(|(u, n, _)| (u, n)).collect())
        .unwrap_or_default()
}

/// Sessions owned by `name` via logind ListSessions.
fn sessions_of(name: &str) -> usize {
    Connection::system()
        .and_then(|c| {
            c.call_method(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                Some("org.freedesktop.login1.Manager"),
                "ListSessions",
                &(),
            )
        })
        .and_then(|m| {
            m.body()
                .deserialize::<Vec<(String, u32, String, String,
                                    zbus::zvariant::OwnedObjectPath)>>()
                .map_err(zbus::Error::from)
        })
        .map(|v| v.iter().filter(|s| s.2 == name).count())
        .unwrap_or(0)
}
