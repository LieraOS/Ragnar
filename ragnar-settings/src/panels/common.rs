// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — shared helpers for embedded panels
// Copyright (C) 2026 LieraOS Project
//
// The recurring widget skeleton of the hub panels (heading page,
// boxed-list card, title+subtitle row, dim caption) plus one
// detached-process helper. Older panels keep their own copies of
// these helpers for now — migrating them is a TODO.md entry, new
// panels use this module from the start.

use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::process::Command;

/// Panel page: vertical box with the standard margins and a
/// heading-styled title label on top.
pub fn page(title: &str) -> Box {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let heading = gtk4::Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);
    page
}

/// Section heading + content container (for sub-groups below the
/// page heading).
pub fn section(title: &str) -> Box {
    let v = Box::new(Orientation::Vertical, 8);
    let h = gtk4::Label::new(Some(title));
    h.add_css_class("heading");
    h.set_xalign(0.0);
    v.append(&h);
    v
}

/// Card-style list (Adwaita boxed-list look).
pub fn card() -> ListBox {
    let l = ListBox::new();
    l.set_selection_mode(gtk4::SelectionMode::None);
    l.add_css_class("boxed-list");
    l
}

/// Card row: title + wrapping subtitle on the left, control on the
/// right.
pub fn row(title: &str, subtitle: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 12);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 12);
    }
    let texts = Box::new(Orientation::Vertical, 2);
    texts.set_hexpand(true);
    texts.set_valign(gtk4::Align::Center);
    let t = gtk4::Label::new(Some(title));
    t.set_xalign(0.0);
    texts.append(&t);
    if !subtitle.is_empty() {
        let s = gtk4::Label::new(Some(subtitle));
        s.add_css_class("dim-label");
        s.add_css_class("caption");
        s.set_xalign(0.0);
        s.set_wrap(true);
        texts.append(&s);
    }
    inner.append(&texts);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Dim, wrapping caption note placed below a card.
pub fn note(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.add_css_class("dim-label");
    l.set_xalign(0.0);
    l.set_wrap(true);
    l
}

/// Spawn `program args..` detached and return immediately; a short
/// thread reaps the child on exit so it cannot linger as a zombie.
/// Needed for arg-array launches where `launch::launch_program`'s
/// whitespace split would break embedded quoting.
pub fn spawn_detached(program: &str, args: &[&str]) -> std::io::Result<()> {
    let child = Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait_with_output();
    });
    Ok(())
}

/// Look up a gsettings schema, returning None instead of panicking —
/// gio::Settings::new aborts on an unknown schema id, so every
/// Settings consumer must check first.
pub fn schema_lookup(schema_id: &str, key: &str) -> Option<gtk4::gio::Settings> {
    let schema = gtk4::gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(schema_id, true))
        .filter(|s| s.list_keys().iter().any(|k| k == key))?;
    Some(gtk4::gio::Settings::new_full(
        &schema,
        None::<&gtk4::gio::SettingsBackend>,
        None,
    ))
}

/// Boolean variant for callers that only need availability.
#[allow(dead_code)]
pub fn schema_exists(schema_id: &str, key: &str) -> bool {
    schema_lookup(schema_id, key).is_some()
}
