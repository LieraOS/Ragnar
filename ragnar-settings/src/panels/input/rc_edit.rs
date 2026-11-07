// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — labwc rc.xml editor for input settings
// Copyright (C) 2026 LieraOS Project
//
// Text-based patcher for the compositor's <libinput> and
// <keyboard> blocks. labwc reloads the file on `labwc -r`, so the
// written values take effect without a restart. Keys are updated
// in place when present and appended inside the block otherwise —
// the surrounding keybinds/menus stay untouched.

use std::path::PathBuf;

/// Path to the user rc.xml (ragnar-desktop keeps labwc's config in
/// its own config dir rather than ~/.config/labwc).
fn rc_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-desktop");
    p.push("labwc");
    p.push("rc.xml");
    p
}

/// Read the whole rc.xml (empty string when unreadable).
fn read() -> String {
    std::fs::read_to_string(rc_path()).unwrap_or_default()
}

/// Replace `<key>v</key>` inside the block spanning
/// `start_tag .. end_tag`; append it before `end_tag` when absent.
fn set_in_block(text: &mut String, start: &str, end: &str, key: &str, val: &str) {
    let Some(a) = text.find(start) else { return };
    let Some(b) = text[a..].find(end).map(|i| a + i) else {
        return;
    };
    let block = &text[a..b];
    let open = format!("<{key}>");
    if let Some(k) = block.find(&open) {
        // Replace the existing element's text.
        let val_start = a + k + open.len();
        let close = format!("</{key}>");
        if let Some(rel) = text[val_start..b].find(&close) {
            text.replace_range(val_start..val_start + rel, val);
            return;
        }
    }
    // Key absent — insert before the block's closing tag.
    text.insert_str(b, &format!("\t\t\t<{key}>{val}</{key}>\n"));
}

/// Write a key inside `<device category="cat">…</device>`.
pub fn set_device(cat: &str, key: &str, val: &str) {
    let mut t = read();
    if t.is_empty() {
        return;
    }
    let start = format!("<device category=\"{cat}\">");
    set_in_block(&mut t, &start, "</device>", key, val);
    let _ = std::fs::write(rc_path(), t);
}

/// Write a key inside `<keyboard>…</keyboard>`.
pub fn set_keyboard(key: &str, val: &str) {
    let mut t = read();
    if t.is_empty() {
        return;
    }
    set_in_block(&mut t, "<keyboard>", "</keyboard>", key, val);
    let _ = std::fs::write(rc_path(), t);
}

/// Read a `<key>` value inside a `<device category>` block.
pub fn get_device(cat: &str, key: &str) -> Option<String> {
    let t = read();
    let a = t.find(&format!("<device category=\"{cat}\">"))?;
    let b = t[a..].find("</device>").map(|i| a + i)?;
    get_tag(&t[a..b], key)
}

/// Read a `<key>` value inside `<keyboard>`.
pub fn get_keyboard(key: &str) -> Option<String> {
    let t = read();
    let a = t.find("<keyboard>")?;
    let b = t[a..].find("</keyboard>").map(|i| a + i)?;
    get_tag(&t[a..b], key)
}

/// Extract `<key>value</key>` from a block snippet.
fn get_tag(block: &str, key: &str) -> Option<String> {
    let open = format!("<{key}>");
    let close = format!("</{key}>");
    let a = block.find(&open)? + open.len();
    let b = block[a..].find(&close).map(|i| a + i)?;
    Some(block[a..b].trim().to_string())
}

/// Ask labwc to reload its configuration — makes edits live.
pub fn reconfigure() {
    let _ = std::process::Command::new("labwc").arg("-r").status();
}
