// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-clipboard — the history picker window.
//
// GTK4 window listing the stored clipboard history; Enter or a
// double click sends `set <id>` to the running daemon (which then
// owns the new clipboard source and can outlive this window).
// Escape or focus loss closes the picker.

use crate::{daemon, i18n, i18n::tr, store};
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk, glib};

/// Format the capture timestamp for the row subtitle.
fn fmt_ts(ts: u64) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_sub(ts);
    if secs < 60 {
        return tr("gerade eben");
    }
    if secs < 3600 {
        return format!("{} min", secs / 60);
    }
    if secs < 86400 {
        return format!("{} h", secs / 3600);
    }
    format!("{} d", secs / 86400)
}

/// Build one list row for an entry.
fn row_for(entry: &store::Entry) -> gtk::ListBoxRow {
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
    vbox.set_margin_top(6);
    vbox.set_margin_bottom(6);
    vbox.set_margin_start(10);
    vbox.set_margin_end(10);

    let preview = gtk::Label::new(Some(&entry.preview));
    preview.set_halign(gtk::Align::Start);
    preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
    vbox.append(&preview);

    let sub = gtk::Label::new(Some(&fmt_ts(entry.ts)));
    sub.set_halign(gtk::Align::Start);
    sub.add_css_class("dim-label");
    sub.add_css_class("caption");
    vbox.append(&sub);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&vbox));
    row.set_widget_name(&entry.id);
    row
}

/// Ask the daemon to install this entry as the clipboard, then quit.
fn activate(app: &gtk::Application, id: &str) {
    match daemon::send(&format!("set {id}")) {
        Ok(r) if r == "ok" => {}
        Ok(r) => eprintln!("ragnar-clipboard: {r}"),
        Err(e) => eprintln!("ragnar-clipboard: daemon unreachable: {e}"),
    }
    app.quit();
}

/// Run the picker window.
pub fn run() -> i32 {
    i18n::init();
    let app = gtk::Application::new(
        Some("org.lieraos.ragnar.Clipboard"),
        Default::default(),
    );
    app.connect_activate(|app| {
        let entries = store::load();

        let win = gtk::Window::builder()
            .application(app)
            .title(&tr("Zwischenablage-Verlauf"))
            .default_width(420)
            .default_height(480)
            .decorated(true)
            .build();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_margin_top(8);
        root.set_margin_bottom(8);
        root.set_margin_start(8);
        root.set_margin_end(8);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&tr("Suchen…")));
        root.append(&search);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        for e in &entries {
            list.append(&row_for(e));
        }
        if entries.is_empty() {
            let empty = gtk::Label::new(Some(&tr("Der Verlauf ist leer.")));
            empty.set_margin_top(20);
            list.append(&empty);
        }

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_child(Some(&list));
        root.append(&scroll);

        // Filter rows by search text — rows are index-aligned with
        // the entries vector.
        {
            let list = list.clone();
            let previews: Vec<String> = entries.iter().map(|e| e.preview.clone()).collect();
            search.connect_changed(move |se| {
                let needle = se.text().to_lowercase();
                let mut i = 0;
                while let Some(row) = list.row_at_index(i) {
                    let visible = needle.is_empty()
                        || previews
                            .get(i as usize)
                            .map(|p| p.to_lowercase().contains(&needle))
                            .unwrap_or(false);
                    row.set_visible(visible);
                    i += 1;
                }
            });
        }

        // Enter / double-click on a row → daemon `set <id>` → quit.
        {
            let app = app.clone();
            list.connect_row_activated(move |_, row| {
                activate(&app, &row.widget_name());
            });
        }
        let click = gtk::GestureClick::new();
        {
            let list = list.clone();
            let app = app.clone();
            click.set_button(1);
            click.connect_pressed(move |_, n, _, _| {
                if n == 2 {
                    if let Some(row) = list.selected_row() {
                        activate(&app, &row.widget_name());
                    }
                }
            });
        }
        list.add_controller(click);

        // Escape closes.
        let keys = gtk::EventControllerKey::new();
        {
            let app = app.clone();
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    app.quit();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        win.add_controller(keys);

        win.set_child(Some(&root));
        if let Some(first) = list.row_at_index(0) {
            list.select_row(Some(&first));
        }
        search.grab_focus();
        win.present();
    });
    // Pass no args: argv[1] ("pick") would otherwise be treated as
    // a file to open, which this application does not handle.
    app.run_with_args(&[] as &[&str]).into()
}
