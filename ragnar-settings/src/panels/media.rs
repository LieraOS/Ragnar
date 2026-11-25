// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Removable media" panel
// Copyright (C) 2026 LieraOS Project
//
// Two parts: the automount kill switch (GSettings key
// org.lieraos.ragnar.automount enabled, shipped by the
// ragnar-settings package and read live by the ragnar-automount
// daemon on every hot-plug event) and a live list of currently
// mounted removable volumes with per-device unmount buttons, fed by
// gio::VolumeMonitor.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{gio, glib, Box, ListBox, Orientation};
use std::cell::RefCell;
use std::path::Path;

/// GSettings location of the automount switch — the daemon re-reads
/// it per event, so toggling applies without a session restart.
const SCHEMA: &str = "org.lieraos.ragnar.automount";
const KEY: &str = "enabled";

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let heading = gtk4::Label::new(Some(&gettext("Removable media")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    // -- Automount switch --------------------------------------------------
    let card = card_list();
    card.append(&automount_row());
    page.append(&card);

    // -- Connected devices -------------------------------------------------
    let dev = section(&gettext("Connected devices"));
    let list = card_list();
    let monitor = gio::VolumeMonitor::get();
    rebuild_devices(&list, &monitor);
    // Keep the list live while the panel exists: plug/unplug and
    // mount/unmount events rebuild the rows. The handlers reference
    // the list weakly and are disconnected when the page is
    // destroyed — the window rebuilds panels on every visit, so
    // strong refs + stale handlers would leak every dead ListBox
    // and keep rebuilding it.
    let weak = list.downgrade();
    let mon = monitor.clone();
    let added = monitor.connect_mount_added(move |_, _| {
        let Some(l) = weak.upgrade() else { return };
        rebuild_devices(&l, &mon);
    });
    let weak = list.downgrade();
    let mon = monitor.clone();
    let removed = monitor.connect_mount_removed(move |_, _| {
        let Some(l) = weak.upgrade() else { return };
        rebuild_devices(&l, &mon);
    });
    // GtkWidget::destroy is emitted from gtk_widget_dispose when the
    // panel's last reference drops (window.rs replace_page removes
    // the page from the stack). SignalHandlerId is not Clone, so the
    // ids travel into the closure through a RefCell.
    let ids = RefCell::new(vec![added, removed]);
    let mon = monitor.clone();
    page.connect_destroy(move |_| {
        for id in ids.take() {
            mon.disconnect(id);
        }
    });
    dev.append(&list);
    page.append(&dev);

    let note = gtk4::Label::new(Some(&gettext(
        "Automount changes apply immediately — devices already \
         mounted stay mounted until removed.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    page.upcast()
}

/// Card row with the automount switch bound to the gsettings key.
/// When the schema is not installed (daemon/package missing) the row
/// renders insensitive instead of crashing — gio::Settings::new
/// panics on an unknown schema id, so the lookup happens first.
fn automount_row() -> gtk4::ListBoxRow {
    let sw = gtk4::Switch::new();
    sw.set_valign(gtk4::Align::Center);
    let schema = gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(SCHEMA, true))
        .filter(|s| s.list_keys().iter().any(|k| k == KEY));
    match schema {
        Some(schema) => {
            let s = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
            sw.set_active(s.boolean(KEY));
            sw.connect_state_set(move |_, on| {
                let _ = s.set_boolean(KEY, on);
                glib::Propagation::Proceed
            });
            row(
                &gettext("Automatically mount removable media"),
                &gettext("Mount USB drives and discs as soon as they are plugged in"),
                sw.upcast(),
            )
        }
        None => {
            let r = row(
                &gettext("Automatically mount removable media"),
                &gettext(
                    "Unavailable: the org.lieraos.ragnar.automount \
                          schema is not installed on this system.",
                ),
                sw.upcast(),
            );
            r.set_sensitive(false);
            r
        }
    }
}

/// Refill `list` with one row per mounted removable volume, or a
/// single dim empty-state row when nothing removable is mounted.
fn rebuild_devices(list: &ListBox, monitor: &gio::VolumeMonitor) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut any = false;
    for mount in monitor.mounts() {
        if !is_removable(&mount) {
            continue;
        }
        any = true;
        list.append(&device_row(&mount, list, monitor));
    }
    if !any {
        let r = gtk4::ListBoxRow::new();
        let l = gtk4::Label::new(Some(&gettext("No removable devices connected")));
        l.add_css_class("dim-label");
        l.set_margin_top(12);
        l.set_margin_bottom(12);
        r.set_child(Some(&l));
        r.set_sensitive(false);
        list.append(&r);
    }
}

/// A mount counts as removable when its drive reports removable media
/// or when it sits under the standard user-mount directories.
fn is_removable(mount: &gio::Mount) -> bool {
    let by_drive = mount
        .volume()
        .and_then(|v| v.drive())
        .map(|d| d.is_removable() || d.is_media_removable())
        .unwrap_or(false);
    let by_path = mount
        .root()
        .path()
        .map(|p| p.starts_with(Path::new("/run/media")) || p.starts_with(Path::new("/media")))
        .unwrap_or(false);
    by_drive || by_path
}

/// Device row: volume name + mount path left, unmount/eject button
/// right. The unmount runs async through the portal-free
/// MountOperation so the UI never blocks.
fn device_row(
    mount: &gio::Mount,
    list: &ListBox,
    monitor: &gio::VolumeMonitor,
) -> gtk4::ListBoxRow {
    let label = if mount.can_eject() {
        gettext("Eject")
    } else {
        gettext("Unmount")
    };
    let btn = gtk4::Button::with_label(&label);
    btn.set_valign(gtk4::Align::Center);
    let (m, l, mon) = (mount.clone(), list.clone(), monitor.clone());
    btn.connect_clicked(move |_| {
        let (m, l, mon) = (m.clone(), l.clone(), mon.clone());
        glib::MainContext::default().spawn_local(async move {
            let op = gio::MountOperation::new();
            let ok = m
                .unmount_with_operation_future(gio::MountUnmountFlags::NONE, Some(&op))
                .await
                .is_ok();
            if ok {
                // Hide the row right away; the mount-removed signal
                // will rebuild again once gio confirms the removal.
                rebuild_devices(&l, &mon);
            }
        });
    });
    let path = mount
        .root()
        .path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| mount.root().uri().to_string());
    row(&mount.name(), &path, btn.upcast())
}

/// Section heading + content container.
fn section(title: &str) -> Box {
    let v = Box::new(Orientation::Vertical, 8);
    let h = gtk4::Label::new(Some(title));
    h.add_css_class("heading");
    h.set_xalign(0.0);
    v.append(&h);
    v
}

/// Card-style list.
fn card_list() -> ListBox {
    let l = ListBox::new();
    l.set_selection_mode(gtk4::SelectionMode::None);
    l.add_css_class("boxed-list");
    l
}

/// Card row: title + subtitle left, control right.
fn row(title: &str, sub: &str, control: gtk4::Widget) -> gtk4::ListBoxRow {
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
    let s = gtk4::Label::new(Some(sub));
    s.add_css_class("dim-label");
    s.add_css_class("caption");
    s.set_xalign(0.0);
    s.set_wrap(true);
    texts.append(&s);
    inner.append(&texts);
    inner.append(&control);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}
