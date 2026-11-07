// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Panel and desktop" panel: autostart manager
// Copyright (C) 2026 LieraOS Project
//
// Lists every XDG autostart entry (system /etc/xdg/autostart plus
// user ~/.config/autostart) with a switch. The mechanism is the
// freedesktop standard: disabling a system entry writes a user-side
// override carrying Hidden=true; re-enabling drops the override
// again. User-owned entries toggle Hidden in place. Taskbar layout
// itself lives in the taskbar's own settings window (Taskbar tile).

use super::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::path::PathBuf;

/// One autostart .desktop entry with its effective state.
struct Entry {
    /// Desktop file id (filename).
    id: String,
    /// Name= value, falls back to the id.
    name: String,
    /// Exec= value, shown dimmed.
    exec: String,
    /// true when this file lives in the user dir (override or own).
    user_owned: bool,
    /// Effective enabled state after system+user resolution.
    enabled: bool,
}

/// Session-critical autostart entries hidden from the list —
/// flipping these would break the desktop (compositor bridge,
/// portal stack, polkit agent, keyring, display/notify services).
/// Substring match against the desktop-file id.
const HIDDEN: &[&str] = &[
    "org.lieraos.ragnar.",
    "ragnar",
    "liera-automount",
    "lieratask-hotkey",
    "cinnamon-settings-daemon",
    "gnome-keyring",
    "at-spi",
    "xdg-user-dirs",
    "pulseaudio",
    "pipewire",
    "geoclue",
    "org.gnome.SettingsDaemon",
    "org.gnome.Evolution-alarm-notify",
    "lieraos-installer",
    "liera-first-run",
];

/// Whether an entry id is user-toggleable at all.
fn toggleable(id: &str) -> bool {
    !HIDDEN.iter().any(|h| id.contains(h))
}

/// User autostart dir.
fn user_dir() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("autostart");
    p
}

const VIEW_SCHEMA: &str = "org.lieraos.ragnar.view";

/// Icon context-menu entries the user may hide: (id, label).
const MENU_ENTRIES: &[(&str, &str)] = &[
    ("open-with", "Open With…"),
    ("terminal", "Open in Terminal"),
    ("rename", "Rename…"),
];

/// "Desktop icons" section: what right-click → delete does and which
/// entries the icon menu shows. Backed by the org.lieraos.ragnar.view
/// schema of ragnar-view; None when that package is absent.
fn icon_menu_section() -> Option<Box> {
    let s = common::schema_lookup(VIEW_SCHEMA, "delete-action")?;
    let sec = common::section(&gettext("Desktop icons"));

    // Delete behaviour — two radio rows.
    let card = common::card();
    let group = gtk4::CheckButton::new();
    let current = s.string("delete-action");
    for (value, title, sub) in [
        (
            "delete",
            gettext("Delete permanently"),
            gettext("Asks for confirmation, then removes the item for good."),
        ),
        (
            "trash",
            gettext("Move to trash"),
            gettext("Keeps deleted items in the trash so they can be restored."),
        ),
    ] {
        let cb = gtk4::CheckButton::new();
        cb.set_group(Some(&group));
        cb.set_active(current == value);
        let s2 = s.clone();
        cb.connect_toggled(move |b| {
            if b.is_active() {
                let _ = s2.set_string("delete-action", value);
            }
        });
        card.append(&common::row(&title, &sub, cb.upcast()));
    }
    sec.append(&card);

    // Visibility switches for the optional menu entries.
    let menu = common::card();
    let hidden: Vec<String> = s
        .strv("context-menu-hidden")
        .iter()
        .map(|x| x.to_string())
        .collect();
    for (id, label) in MENU_ENTRIES {
        let sw = gtk4::Switch::new();
        sw.set_valign(gtk4::Align::Center);
        sw.set_active(!hidden.iter().any(|h| h == id));
        let s2 = s.clone();
        sw.connect_state_set(move |_, on| {
            let mut list: Vec<String> = s2
                .strv("context-menu-hidden")
                .iter()
                .map(|x| x.to_string())
                .collect();
            list.retain(|h| h != id);
            if !on {
                list.push((*id).to_string());
            }
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            let _ = s2.set_strv("context-menu-hidden", refs);
            gtk4::glib::Propagation::Proceed
        });
        menu.append(&common::row(&gettext(*label), "", sw.upcast()));
    }
    let title = gtk4::Label::new(Some(&gettext("Entries in the icon menu")));
    title.set_xalign(0.0);
    title.add_css_class("dim-label");
    sec.append(&title);
    sec.append(&menu);
    Some(sec)
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 16);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    if let Some(sec) = icon_menu_section() {
        page.append(&sec);
    }

    let heading = gtk4::Label::new(Some(&gettext("Autostart")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    let card = ListBox::new();
    card.set_selection_mode(gtk4::SelectionMode::None);
    card.add_css_class("boxed-list");
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&card));
    page.append(&scroll);

    fill(&card);

    let hint = gtk4::Label::new(Some(&gettext(
        "Disabled entries are hidden per user — the system file is \
         never modified.",
    )));
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    page.append(&hint);

    page.upcast()
}

/// Read a key from a .desktop file's [Desktop Entry] section.
fn key(path: &PathBuf, want: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with(&format!("{want}=")))
            .map(|l| l.splitn(2, '=').nth(1).unwrap_or("").trim().to_string())
    })
}

/// Collect entries — user dir overlays system dir by filename.
fn collect() -> Vec<Entry> {
    let mut by_id: std::collections::BTreeMap<String, Entry> = Default::default();
    let udir = user_dir();
    for (dir, user_owned) in [(PathBuf::from("/etc/xdg/autostart"), false), (udir, true)] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x != "desktop").unwrap_or(true) {
                continue;
            }
            let id = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !toggleable(&id) {
                continue;
            }
            let hidden = key(&p, "Hidden").as_deref() == Some("true");
            let xg = key(&p, "X-GNOME-Autostart-enabled").as_deref() == Some("false");
            let name = key(&p, "Name")
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| id.clone());
            let exec = key(&p, "Exec").unwrap_or_default();
            match by_id.get_mut(&id) {
                // User file overrides the system entry's state.
                Some(prev) if user_owned => {
                    prev.enabled = !(hidden || xg);
                    prev.user_owned = true;
                }
                // Only insert when not already seen — first win keeps
                // the user copy's name/exec too.
                None => {
                    by_id.insert(
                        id.clone(),
                        Entry {
                            id,
                            name,
                            exec,
                            user_owned,
                            enabled: !(hidden || xg),
                        },
                    );
                }
                _ => {}
            }
        }
    }
    by_id.into_values().collect()
}

/// Re-render the rows.
fn fill(card: &ListBox) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    for e in collect() {
        let inner = Box::new(Orientation::Horizontal, 12);
        for m in ["top", "bottom", "start", "end"] {
            inner.set_property(&format!("margin-{m}"), 10);
        }
        let texts = Box::new(Orientation::Vertical, 2);
        texts.set_hexpand(true);
        texts.set_valign(gtk4::Align::Center);
        let t = gtk4::Label::new(Some(&e.name));
        t.set_xalign(0.0);
        texts.append(&t);
        if !e.exec.is_empty() {
            let x = gtk4::Label::new(Some(&e.exec));
            x.add_css_class("dim-label");
            x.add_css_class("caption");
            x.set_xalign(0.0);
            x.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            texts.append(&x);
        }
        inner.append(&texts);

        let sw = gtk4::Switch::new();
        sw.set_active(e.enabled);
        sw.set_valign(gtk4::Align::Center);
        let id = e.id.clone();
        sw.connect_state_set(move |_, on| {
            toggle(&id, on);
            gtk4::glib::Propagation::Proceed
        });
        inner.append(&sw);

        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&inner));
        card.append(&row);
    }
}

/// Toggle entry `id`: system files get a Hidden override written
/// into the user dir; user-owned files flip Hidden in place.
fn toggle(id: &str, enable: bool) {
    let upath = user_dir().join(id);
    let spath = PathBuf::from("/etc/xdg/autostart").join(id);
    if upath.exists() {
        // User copy already exists — set/clear its Hidden flag,
        // remove the file entirely when it only carried the flag.
        let body = std::fs::read_to_string(&upath).unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        let mut wrote = false;
        for l in body.lines() {
            if l.starts_with("Hidden=") {
                out.push(format!("Hidden={}", !enable));
                wrote = true;
            } else {
                out.push(l.to_string());
            }
        }
        if !wrote {
            out.push(format!("Hidden={}", !enable));
        }
        let _ = std::fs::write(&upath, out.join("\n") + "\n");
        return;
    }
    if enable || !spath.exists() {
        return; // nothing to override, or no source at all
    }
    // Disable a system entry — user override with Hidden=true.
    if let Some(d) = upath.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut body = std::fs::read_to_string(&spath).unwrap_or_default();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("Hidden=true\n");
    let _ = std::fs::write(&upath, body);
}
