// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings-gui — appearance sub-page: icon theme
// Copyright (C) 2026 LieraOS Project
//
// Dropdown of installed icon themes writing
// org.gnome.desktop.interface icon-theme, plus "Install from file".

use gettextrs::gettext;
use gtk4::prelude::*;

use super::common;

/// Build the icon theme page.
pub fn build() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    page.set_margin_top(18);

    let names = common::list_icon_themes();
    let iface = gtk4::gio::Settings::new("org.gnome.desktop.interface");
    let current = iface.string("icon-theme").to_string();

    let dd = gtk4::DropDown::from_strings(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    if let Some(i) = names.iter().position(|n| *n == current) {
        dd.set_selected(i as u32);
    }
    dd.set_hexpand(true);
    dd.connect_selected_notify(move |d| {
        if let Some(name) = names.get(d.selected() as usize) {
            let _ = iface.set_string("icon-theme", name);
        }
    });
    page.append(&dd);

    let status = gtk4::Label::new(None);
    status.add_css_class("dim-label");

    let btn = gtk4::Button::with_label(&gettext("Install icon theme from file…"));
    {
        let status = status.clone();
        btn.connect_clicked(move |b| {
            let Some(win) = b.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
                return;
            };
            let dialog = gtk4::FileDialog::new();
            dialog.set_title(&gettext("Choose icon theme archive"));
            let status = status.clone();
            dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |res| {
                if let Ok(file) = res {
                    let path = file.path().unwrap_or_default();
                    match common::install_archive(&path, "icons") {
                        Ok(()) => status.set_label(&gettext("Installed. Select it above.")),
                        Err(e) => {
                            status.set_label(&format!("{}: {}", gettext("Install failed"), e))
                        }
                    }
                }
            });
        });
    }
    page.append(&btn);
    page.append(&status);

    page
}
