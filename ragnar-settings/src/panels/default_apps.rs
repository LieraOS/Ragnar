// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings-gui — default applications module
// Copyright (C) 2026 LieraOS Project
//
// Real default-app mechanism: xdg-mime default <desktop-id> <mime>.
// Candidates are enumerated by scanning .desktop files that declare
// the matching MimeType. Terminal uses the ragnar-view gsettings key.

use crate::i18n::msgid;
use gettextrs::gettext;
use gtk4::prelude::*;
use std::collections::BTreeMap;
use std::process::Command;

/// (label, mime types) per category.
const CATEGORIES: &[(&str, &[&str])] = &[
    (
        msgid("Web browser"),
        &["x-scheme-handler/http", "x-scheme-handler/https"],
    ),
    (msgid("Email"), &["x-scheme-handler/mailto"]),
    (msgid("Calendar"), &["text/calendar"]),
    (msgid("File manager"), &["inode/directory"]),
    (msgid("Text editor"), &["text/plain"]),
    (msgid("Archive manager"), &["application/zip"]),
    (msgid("Image viewer"), &["image/png", "image/jpeg"]),
    (msgid("Video player"), &["video/mp4"]),
    (msgid("Music player"), &["audio/mpeg"]),
    (msgid("PDF viewer"), &["application/pdf"]),
];

/// Create the Default Apps page.
pub fn build() -> gtk4::Widget {
    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 18);
    container.set_margin_start(24);
    container.set_margin_end(24);
    container.set_margin_top(24);
    container.set_margin_bottom(24);

    let title = gtk4::Label::new(Some(&gettext("Default Apps")));
    title.add_css_class("title-1");
    title.set_halign(gtk4::Align::Start);
    container.append(&title);

    for (label, mimes) in CATEGORIES {
        container.append(&category_row(label, mimes));
    }

    container.upcast()
}

/// One category row: label + dropdown of candidate apps.
fn category_row(label: &str, mimes: &[&str]) -> gtk4::Box {
    let mut candidates = apps_for_mimes(mimes);
    let current = current_default(mimes);

    // The current default may be a hidden desktop id (e.g. a stale
    // `userapp-*` file Firefox registered itself under). Excluded from
    // candidates by NoDisplay, position() would find nothing and the
    // DropDown would silently show index 0 — a wrong app. Re-add the
    // hidden default with its real name so the display stays truthful.
    if !current.is_empty() && !candidates.iter().any(|(id, _)| *id == current) {
        if let Some(name) = desktop_name(&current) {
            candidates.push((current.clone(), name));
        }
    }

    let dd = gtk4::DropDown::from_strings(
        &candidates
            .iter()
            .map(|(_, n)| n.as_str())
            .collect::<Vec<_>>(),
    );
    if let Some(i) = candidates.iter().position(|(id, _)| *id == current) {
        dd.set_selected(i as u32);
    }
    dd.set_hexpand(true);

    let mimes_owned: Vec<String> = mimes.iter().map(|m| m.to_string()).collect();
    dd.connect_selected_notify(move |d| {
        if let Some((id, _)) = candidates.get(d.selected() as usize) {
            for mime in &mimes_owned {
                let _ = Command::new("xdg-mime")
                    .args(["default", id, mime])
                    .output();
            }
        }
    });

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(&gettext(label))));
    dd.set_halign(gtk4::Align::End);
    row.append(&dd);
    row
}

/// Enumerate .desktop apps declaring any of the mimes. Returns (id, name).
/// Only the `[Desktop Entry]` group is parsed — action sections like
/// "[Desktop Action profile-manager]" carry their own Name= lines and
/// must not overwrite the app's real name.
fn apps_for_mimes(mimes: &[&str]) -> Vec<(String, String)> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        "/usr/share/applications".to_string(),
        format!("{home}/.local/share/applications"),
        "/var/lib/flatpak/exports/share/applications".to_string(),
        format!("{home}/.local/share/flatpak/exports/share/applications"),
        "/var/lib/snapd/desktop/applications".to_string(),
    ];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x != "desktop").unwrap_or(true) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            if text.contains("NoDisplay=true") || text.contains("Hidden=true") {
                continue;
            }
            let mut name = String::new();
            let mut mime_line = String::new();
            let mut in_entry = false;
            for line in text.lines() {
                if line.starts_with('[') {
                    in_entry = line == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = line.strip_prefix("Name=") {
                    name = v.to_string();
                }
                if let Some(v) = line.strip_prefix("MimeType=") {
                    mime_line = v.to_string();
                }
            }
            if name.is_empty() || mime_line.is_empty() {
                continue;
            }
            let id = e.file_name().to_string_lossy().to_string();
            // "Trash"/"Papierkorb" handlers are not file managers.
            if name.to_lowercase().contains("trash")
                || name.contains("Papierkorb")
                || id.to_lowercase().contains("trash")
            {
                continue;
            }
            let hit = mime_line.split(';').any(|m| mimes.contains(&m.trim()));
            if hit {
                out.insert(id, name);
            }
        }
    }
    out.into_iter().collect()
}

/// Read the `Name=` of a .desktop file by id across the app dirs —
/// used to label a hidden default that is absent from candidates.
fn desktop_name(id: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        "/usr/share/applications".to_string(),
        format!("{home}/.local/share/applications"),
        "/var/lib/flatpak/exports/share/applications".to_string(),
        format!("{home}/.local/share/flatpak/exports/share/applications"),
        "/var/lib/snapd/desktop/applications".to_string(),
    ];
    for dir in dirs {
        let path = format!("{dir}/{id}");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut in_entry = false;
        for line in text.lines() {
            if line.starts_with('[') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if in_entry {
                if let Some(v) = line.strip_prefix("Name=") {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Current default app id for the first mime (via xdg-mime query).
fn current_default(mimes: &[&str]) -> String {
    let mime = mimes.first().copied().unwrap_or("");
    let out = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}
