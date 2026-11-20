// SPDX-License-Identifier: GPL-3.0-or-later
//! Startup tab — XDG autostart entries from /etc/xdg/autostart and
//! the user's own autostart dir. Toggling writes a Hidden=true key
//! into the per-user copy, which masks the system entry per the XDG
//! autostart spec — the system file itself is never touched.

use std::fs;
use std::path::PathBuf;

use gtk4::prelude::*;

/// One autostart entry resolved across system+user dirs.
struct Entry {
    file: String, // desktop file basename
    name: String,
    comment: String,
    enabled: bool,
}

/// Build the startup page.
pub fn page() -> gtk4::Widget {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .build();
    root.append(&scroll);

    rebuild(&list);
    root.upcast()
}

/// Merge system and user autostart dirs, user copy wins per file.
fn entries() -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let user_dir = user_dir();
    let sys_dir = PathBuf::from("/etc/xdg/autostart");
    let mut seen = std::collections::HashSet::new();
    for dir in [&user_dir, &sys_dir] {
        let Ok(rd) = fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            let file = e.file_name().to_string_lossy().into_owned();
            if seen.contains(&file) {
                continue;
            }
            seen.insert(file.clone());
            if let Some(en) = parse(&p, &file) {
                out.push(en)
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Parse one .desktop: Name/Comment and the effective enabled flag.
fn parse(p: &std::path::Path, file: &str) -> Option<Entry> {
    let s = fs::read_to_string(p).ok()?;
    let get = |k: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(|v| v.trim().to_string())
    };
    if get("NoDisplay").as_deref() == Some("true") {
        return None;
    }
    let enabled = get("Hidden").as_deref() != Some("true")
        && get("X-GNOME-Autostart-enabled").as_deref() != Some("false");
    Some(Entry {
        file: file.into(),
        name: get("Name").unwrap_or_else(|| file.trim_end_matches(".desktop").into()),
        comment: get("Comment").unwrap_or_default(),
        enabled,
    })
}

/// Rebuild the list rows.
fn rebuild(list: &gtk4::ListBox) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    for en in entries() {
        list.append(&row(en, list));
    }
}

/// One row: name+comment left, on/off switch right.
fn row(en: Entry, list: &gtk4::ListBox) -> gtk4::ListBoxRow {
    let r = gtk4::ListBoxRow::new();
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.set_margin_start(10);
    b.set_margin_end(10);

    let n = gtk4::Label::builder()
        .label(&en.name)
        .halign(gtk4::Align::Start)
        .build();
    n.add_css_class("heading");
    let d = gtk4::Label::builder()
        .label(&en.comment)
        .halign(gtk4::Align::Start)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    d.add_css_class("dim-label");
    d.add_css_class("caption");
    let left = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    left.set_hexpand(true);
    left.append(&n);
    left.append(&d);

    let sw = gtk4::Switch::builder()
        .active(en.enabled)
        .valign(gtk4::Align::Center)
        .build();
    sw.connect_state_set({
        let file = en.file.clone();
        let list = list.clone();
        move |_, on| {
            set_enabled(&file, on);
            rebuild(&list);
            glib::Propagation::Proceed
        }
    });

    b.append(&left);
    b.append(&sw);
    r.set_child(Some(&b));
    r
}

/// Per-user autostart directory (created on demand).
fn user_dir() -> PathBuf {
    let mut p = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    p = p.join("autostart");
    let _ = fs::create_dir_all(&p);
    p
}

/// Enable/disable: ensure a user-local copy exists, then rewrite the
/// Hidden key in it so the system file stays untouched.
fn set_enabled(file: &str, on: bool) {
    let dst = user_dir().join(file);
    if !dst.exists() {
        let src = PathBuf::from("/etc/xdg/autostart").join(file);
        if fs::copy(&src, &dst).is_err() {
            return;
        }
    }
    let Ok(s) = fs::read_to_string(&dst) else {
        return;
    };
    let mut out: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    out.retain(|l| !l.starts_with("Hidden="));
    out.push(format!("Hidden={}", if on { "false" } else { "true" }));
    let _ = fs::write(&dst, out.join("\n") + "\n");
}
