// SPDX-License-Identifier: GPL-3.0-or-later
//! History page: the last packagekitd transactions (updates, installs,
//! removals, local file installs) from GetOldTransactions.

use gettextrs::gettext;
use gtk4::prelude::*;

use crate::packagekit::HistoryEntry;

pub struct HistoryPage {
    pub widget: gtk4::Widget,
    list: gtk4::ListBox,
    status: gtk4::Label,
}

impl HistoryPage {
    pub fn new() -> Self {
        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        let scroll = gtk4::ScrolledWindow::builder()
            .child(&list)
            .hexpand(true)
            .vexpand(true)
            .build();
        let head = gtk4::Label::new(Some(&gettext("History")));
        head.add_css_class("title-3");
        head.set_halign(gtk4::Align::Start);
        head.set_margin_start(8);
        head.set_margin_top(8);
        let status = gtk4::Label::new(Some(&gettext("Loading history …")));
        status.set_halign(gtk4::Align::Start);
        status.set_margin_start(8);
        status.add_css_class("dim-label");
        let col = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        col.append(&head);
        col.append(&status);
        col.append(&scroll);
        Self {
            widget: col.upcast(),
            list,
            status,
        }
    }

    /// Fill the list with fetched transactions (newest first).
    pub fn set_entries(&self, entries: &[HistoryEntry]) {
        while let Some(r) = self.list.row_at_index(0) {
            self.list.remove(&r);
        }
        self.status
            .set_label(&format!("{} {}", entries.len(), gettext("transactions")));
        for e in entries.iter().rev() {
            let t = gtk4::Label::new(Some(&format!(
                "{}  {}",
                e.time.split('T').next().unwrap_or(""),
                e.time
                    .split('T')
                    .nth(1)
                    .and_then(|x| x.split('Z').next())
                    .unwrap_or("")
            )));
            t.set_halign(gtk4::Align::Start);
            t.add_css_class("dim-label");
            let role = gtk4::Label::new(Some(&e.action));
            role.set_halign(gtk4::Align::Start);
            let ok = gtk4::Label::new(Some(if e.ok { "✓" } else { "✗" }));
            ok.set_halign(gtk4::Align::End);

            let pkgs = gtk4::Label::new(Some(&e.names));
            pkgs.set_halign(gtk4::Align::Start);
            pkgs.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            pkgs.add_css_class("dim-label");
            let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            top.append(&t);
            top.append(&role);
            top.append(&ok);
            let v = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            v.set_margin_top(6);
            v.set_margin_bottom(6);
            v.append(&top);
            v.append(&pkgs);
            let row = gtk4::ListBoxRow::new();
            row.set_activatable(false);
            row.set_child(Some(&v));
            self.list.append(&row);
        }
    }

    pub fn set_error(&self, e: &str) {
        self.status
            .set_label(&format!("{}: {e}", gettext("History not available")));
    }
}
