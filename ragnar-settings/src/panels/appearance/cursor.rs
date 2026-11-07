// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — appearance sub-page: cursor theme + size
// Copyright (C) 2026 LieraOS Project
//
// Dropdown of installed cursor themes writing
// org.gnome.desktop.interface cursor-theme, a size slider for
// cursor-size, plus "Install from file".

use gtk4::prelude::*;

use super::common;

/// Build the cursor page.
pub fn build() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    page.set_margin_top(18);

    let names = common::list_cursor_themes();
    let iface = gtk4::gio::Settings::new("org.gnome.desktop.interface");
    let current = iface.string("cursor-theme").to_string();

    let dd = gtk4::DropDown::from_strings(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    if let Some(i) = names.iter().position(|n| *n == current) {
        dd.set_selected(i as u32);
    }
    dd.set_hexpand(true);
    {
        let iface = iface.clone();
        dd.connect_selected_notify(move |d| {
            if let Some(name) = names.get(d.selected() as usize) {
                let _ = iface.set_string("cursor-theme", name);
            }
        });
    }
    page.append(&dd);

    // Cursor size slider (8–96 px).
    let size_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    size_row.append(&gtk4::Label::new(Some("Cursor size")));
    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 8.0, 96.0, 1.0);
    scale.set_value(iface.int("cursor-size") as f64);
    scale.set_hexpand(true);
    scale.set_digits(0);
    scale.set_value_pos(gtk4::PositionType::Right);
    scale.connect_value_changed(move |s| {
        let _ = iface.set_int("cursor-size", s.value() as i32);
    });
    size_row.append(&scale);
    page.append(&size_row);

    let status = gtk4::Label::new(None);
    status.add_css_class("dim-label");

    let btn = gtk4::Button::with_label("Install cursor theme from file…");
    {
        let status = status.clone();
        btn.connect_clicked(move |b| {
            let Some(win) = b.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
                return;
            };
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Choose cursor theme archive");
            let status = status.clone();
            dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |res| {
                if let Ok(file) = res {
                    let path = file.path().unwrap_or_default();
                    match common::install_archive(&path, "icons") {
                        Ok(()) => status.set_label("Installed. Select it above."),
                        Err(e) => status.set_label(&format!("Install failed: {}", e)),
                    }
                }
            });
        });
    }
    page.append(&btn);
    page.append(&status);

    page
}
