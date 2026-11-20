// SPDX-License-Identifier: GPL-3.0-or-later
//! Services tab — systemd units over the standard
//! org.freedesktop.systemd1 D-Bus API (the same interface every
//! service-manager frontend uses; polkit authorisation is handled
//! by systemd itself). Listing and control calls run on worker
//! threads so the UI never blocks behind a polkit prompt.

use gettextrs::gettext as tr;
use gtk4::prelude::*;

use zbus::blocking::Connection;
use zbus::zvariant::OwnedObjectPath;

/// Fields of Manager.ListUnits we display.
type Unit = (String, String, String); // name, description, sub_state

/// Full ListUnits signature a(ssssssouso) — kept as a tuple type.
type RawUnit = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

/// Build the services page: search, list, manual refresh.
pub fn page() -> gtk4::Widget {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let search = gtk4::SearchEntry::builder()
        .hexpand(true)
        .placeholder_text(&tr("Search services"))
        .build();
    let refresh = gtk4::Button::with_label(&tr("Refresh"));
    top.append(&search);
    top.append(&refresh);

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .build();
    let status = gtk4::Label::new(None);
    status.add_css_class("dim-label");

    let (tx, rx) = async_channel::unbounded::<Vec<Unit>>();
    let reload = move |tx: async_channel::Sender<Vec<Unit>>| {
        std::thread::spawn(move || {
            let _ = tx.send_blocking(fetch());
        });
    };

    refresh.connect_clicked({
        let tx = tx.clone();
        move |_| reload(tx.clone())
    });

    // Consume fetch results on the main loop.
    glib::spawn_future_local({
        let list = list.clone();
        let search = search.clone();
        let tx = tx.clone();
        async move {
            while let Ok(units) = rx.recv().await {
                let f = search.text().to_lowercase();
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                for (name, desc, sub) in &units {
                    if !f.is_empty()
                        && !name.to_lowercase().contains(&f)
                        && !desc.to_lowercase().contains(&f)
                    {
                        continue;
                    }
                    list.append(&row(name, desc, sub, tx.clone()));
                }
            }
        }
    });

    reload(tx.clone());
    root.append(&top);
    root.append(&scroll);
    root.append(&status);
    root.upcast()
}

/// One service row: name+description, sub-state, control buttons.
fn row(
    name: &str,
    desc: &str,
    sub: &str,
    tx: async_channel::Sender<Vec<Unit>>,
) -> gtk4::ListBoxRow {
    let r = gtk4::ListBoxRow::new();
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.set_margin_start(10);
    b.set_margin_end(10);

    let n = gtk4::Label::builder()
        .label(name)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    n.add_css_class("heading");
    let d = gtk4::Label::builder()
        .label(desc)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    d.add_css_class("dim-label");
    d.add_css_class("caption");
    let left = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    left.set_hexpand(true);
    left.append(&n);
    left.append(&d);

    let state = gtk4::Label::new(Some(sub));
    if sub == "running" {
        state.add_css_class("success")
    } else {
        state.add_css_class("dim-label")
    }

    b.append(&left);
    b.append(&state);
    for (label, method) in [
        (tr("Start"), "StartUnit"),
        (tr("Stop"), "StopUnit"),
        (tr("Restart"), "RestartUnit"),
    ] {
        let btn = gtk4::Button::with_label(&label);
        btn.connect_clicked({
            let name = name.to_string();
            let tx = tx.clone();
            move |_| {
                let name = name.clone();
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = control(&name, method);
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let _ = tx.send_blocking(fetch());
                });
            }
        });
        b.append(&btn);
    }
    r.set_child(Some(&b));
    r
}

/// ListUnits → filtered, sorted service list.
fn fetch() -> Vec<Unit> {
    let mut out = Vec::new();
    if let Ok(c) = Connection::system() {
        if let Ok(m) = c.call_method(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            Some("org.freedesktop.systemd1.Manager"),
            "ListUnits",
            &(),
        ) {
            if let Ok(units) = m.body().deserialize::<Vec<RawUnit>>() {
                for u in units {
                    if u.0.ends_with(".service") {
                        out.push((u.0, u.1, u.4));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Start/Stop/RestartUnit — polkit raises the auth dialog itself.
fn control(name: &str, method: &'static str) -> Result<(), zbus::Error> {
    let c = Connection::system()?;
    c.call_method(
        Some("org.freedesktop.systemd1"),
        "/org/freedesktop/systemd1",
        Some("org.freedesktop.systemd1.Manager"),
        method,
        &(name, "replace"),
    )?;
    Ok(())
}
