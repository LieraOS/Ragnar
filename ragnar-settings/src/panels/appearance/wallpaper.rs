// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — appearance sub-page: wallpaper
// Copyright (C) 2026 LieraOS Project
//
// Thumbnail grid of every wallpaper found under the system
// (/usr/share/backgrounds) and user (~/.local/share/backgrounds)
// directories. "Choose image file…" copies a custom image into the
// user wallpaper dir so it becomes a normal tile and survives
// removal of the original file. Selecting writes picture-uri +
// picture-uri-dark and the screensaver picture-uri so the lock
// screen follows, then respawns swaybg so the change applies
// immediately.

use gettextrs::gettext;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::wallpaper_map;

/// (tile path, check badge) pairs so the badge can move on select.
type Badges = Rc<RefCell<Vec<(String, gtk4::Label)>>>;

/// Build the wallpaper page.
pub fn build() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    page.set_margin_top(18);

    // Per-monitor selector: "All monitors" writes the global
    // picture-uri; a specific connector assigns an override for
    // that output only.
    let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let flow = gtk4::FlowBox::new();
    flow.set_max_children_per_line(5);
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_homogeneous(true);
    let badges: Badges = Rc::new(RefCell::new(Vec::new()));

    let sel_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sel_row.append(&gtk4::Label::new(Some(&gettext("Apply to:"))));
    let mut names: Vec<String> = vec![gettext("All monitors")];
    names.extend(wallpaper_map::outputs());
    let names_rc: Rc<Vec<String>> = Rc::new(names);
    let dd = gtk4::DropDown::from_strings(&names_rc.iter().map(|n| n.as_str()).collect::<Vec<_>>());
    let reset = gtk4::Button::with_label(&gettext("Use default for this monitor"));
    reset.set_sensitive(false);
    {
        let (sel, fl, bg, nr, rst) = (
            selected.clone(),
            flow.clone(),
            badges.clone(),
            names_rc.clone(),
            reset.clone(),
        );
        dd.connect_selected_notify(move |d| {
            let i = d.selected() as usize;
            *sel.borrow_mut() = nr.get(i).filter(|_| i > 0).cloned();
            rst.set_sensitive(sel.borrow().is_some());
            fill_grid(&fl, &bg, &sel.borrow());
        });
    }
    sel_row.append(&dd);
    {
        let (sel, fl, bg) = (selected.clone(), flow.clone(), badges.clone());
        reset.connect_clicked(move |_| {
            let Some(o) = sel.borrow().clone() else {
                return;
            };
            let mut m = wallpaper_map::load();
            m.remove(&o);
            wallpaper_map::save(&m);
            fill_grid(&fl, &bg, &sel.borrow());
        });
    }
    sel_row.append(&reset);
    page.append(&sel_row);

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(300);

    fill_grid(&flow, &badges, &selected.borrow());

    scrolled.set_child(Some(&flow));
    page.append(&scrolled);

    // Custom image picker — copies the file into the user wallpaper
    // dir, applies it and rebuilds the grid so the new tile shows up.
    let custom = gtk4::Button::with_label(&gettext("Choose image file…"));
    custom.connect_clicked(move |b| {
        let Some(win) = b.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
            return;
        };
        let flow = flow.clone();
        let badges = badges.clone();
        let selected = selected.clone();
        let dialog = gtk4::FileDialog::new();
        dialog.set_title(&gettext("Choose wallpaper image"));
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some(&gettext("Images")));
        for mime in ["image/png", "image/jpeg", "image/webp", "image/avif"] {
            filter.add_mime_type(mime);
        }
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |res| {
            let Ok(file) = res else { return };
            let Some(src) = file.path() else { return };
            let Some(dst) = import_wallpaper(&src) else {
                return;
            };
            set_wallpaper(&dst);
            fill_grid(&flow, &badges, &selected.borrow());
        });
    });
    page.append(&custom);

    page
}

/// (Re)build all tiles — clears the grid and badges, then re-adds
/// one tile per found wallpaper with the check badge on the active
/// one.
fn fill_grid(flow: &gtk4::FlowBox, badges: &Badges, selected: &Option<String>) {
    while let Some(c) = flow.first_child() {
        flow.remove(&c);
    }
    badges.borrow_mut().clear();

    // Decode thumbnails on a worker thread and hand finished
    // pixbufs back over a channel — image decoding never blocks
    // the UI this way.
    let (tx, rx) = async_channel::unbounded::<(usize, Vec<u8>)>();
    let pictures = Rc::new(RefCell::new(Vec::<gtk4::Picture>::new()));
    {
        let pictures = pictures.clone();
        gtk4::glib::MainContext::default().spawn_local(async move {
            while let Ok((i, bytes)) = rx.recv().await {
                if let Some(pic) = pictures.borrow().get(i) {
                    if let Ok(t) = gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from(&bytes))
                    {
                        pic.set_paintable(Some(&t));
                    }
                }
            }
        });
    }

    // The badge marks the image active for the current scope:
    // global picture-uri, or the per-output assignment.
    let current = active_for(selected);
    let selected_scope = Rc::new(RefCell::new(selected.clone()));
    let paths = collect_wallpapers();
    for path in paths.iter() {
        // Placeholder tile — filled in when the worker delivers
        // the decoded thumbnail.
        let pic = gtk4::Picture::new();
        pic.set_size_request(120, 120);
        pic.set_content_fit(gtk4::ContentFit::Contain);
        pic.set_can_shrink(true);
        pictures.borrow_mut().push(pic.clone());

        // Check badge overlaid top-right on the active wallpaper.
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&pic));
        let badge = gtk4::Label::new(Some("✓"));
        badge.add_css_class("accent");
        badge.set_halign(gtk4::Align::End);
        badge.set_valign(gtk4::Align::Start);
        badge.set_margin_end(4);
        badge.set_margin_top(2);
        badge.set_visible(path.to_string_lossy() == current);
        overlay.add_overlay(&badge);
        badges
            .borrow_mut()
            .push((path.to_string_lossy().to_string(), badge));

        let btn = gtk4::Button::new();
        btn.set_child(Some(&overlay));
        btn.set_tooltip_text(Some(
            &path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        ));

        let p = path.clone();
        let badges_c = badges.clone();
        let sel_scope = selected_scope.clone();
        btn.connect_clicked(move |_| {
            apply_to_scope(&p, &sel_scope.borrow());
            // Move the check badge to the clicked tile.
            let sel = p.to_string_lossy().to_string();
            for (tile, badge) in badges_c.borrow().iter() {
                badge.set_visible(*tile == sel);
            }
        });
        flow.append(&btn);
    }

    // Worker: decode all thumbnails off-thread, each result is
    // delivered through the channel above.
    std::thread::spawn(move || {
        for (i, p) in paths.iter().enumerate() {
            if let Some(px) = thumbnail(p) {
                let _ = tx.send_blocking((i, px));
            }
        }
    });
}

/// Scaled thumbnail for `path`, decoded off the main thread.
/// Results are cached under ~/.cache/ragnar-settings/thumbs keyed
/// by path+size+mtime so repeat visits skip decoding entirely.
fn thumbnail(path: &Path) -> Option<Vec<u8>> {
    let cache = thumb_cache_path(path);
    if let Some(c) = &cache {
        if let Ok(bytes) = std::fs::read(c) {
            return Some(bytes);
        }
    }
    let p = gdk_pixbuf::Pixbuf::from_file_at_scale(path, 240, 240, true).ok()?;
    let bytes = p.save_to_bufferv("png", &[]).ok()?;
    if let Some(c) = cache {
        let _ = std::fs::write(c, &bytes);
    }
    Some(bytes)
}

/// Cache file for `path` — changes whenever the source file's
/// size or modification time changes.
fn thumb_cache_path(path: &Path) -> Option<PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut h = DefaultHasher::new();
    (path.to_string_lossy(), meta.len(), mtime).hash(&mut h);
    let mut dir = gtk4::glib::user_cache_dir();
    dir.push("ragnar-settings");
    dir.push("thumbs");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(format!("{}.png", h.finish())))
}

/// Copy a user-picked image into the user wallpaper dir. Returns
/// the destination path; `None` when the copy fails.
fn import_wallpaper(src: &Path) -> Option<PathBuf> {
    let dir = user_wallpaper_dir();
    let _ = std::fs::create_dir_all(&dir);
    let name = src.file_name()?.to_string_lossy().to_string();
    let dst = dir.join(name);
    // Skip the copy when the file already lives there (e.g. picking
    // an image straight out of the wallpaper dir itself).
    if src != dst {
        std::fs::copy(src, &dst).ok()?;
    }
    Some(dst)
}

/// Per-user wallpaper storage (XDG data dir).
fn user_wallpaper_dir() -> PathBuf {
    let mut p = gtk4::glib::user_data_dir();
    p.push("backgrounds");
    p
}

/// Collect image files from system + user wallpaper dirs.
fn collect_wallpapers() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in [
        PathBuf::from("/usr/share/backgrounds"),
        user_wallpaper_dir(),
    ] {
        collect_dir(&root, &mut out);
    }
    out.sort();
    out
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_dir(&p, out);
        } else if let Some(ext) = p.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "avif") {
                out.push(p);
            }
        }
    }
}

/// Current wallpaper path from org.gnome.desktop.background picture-uri.
fn current_wallpaper() -> String {
    let s = gtk4::gio::Settings::new("org.gnome.desktop.background");
    let uri = s.string("picture-uri").to_string();
    uri.trim_start_matches("file://").to_string()
}

/// Image active for the scope shown in the grid.
fn active_for(selected: &Option<String>) -> String {
    match selected {
        None => current_wallpaper(),
        Some(o) => wallpaper_map::load()
            .get(o)
            .cloned()
            .unwrap_or_else(current_wallpaper),
    }
}

/// Assign `path` to the selected scope (all monitors or one output).
fn apply_to_scope(path: &Path, selected: &Option<String>) {
    match selected {
        None => {
            // "All monitors" means literally all — drop every
            // per-output override before applying the global image.
            wallpaper_map::save(&std::collections::BTreeMap::new());
            set_wallpaper(path);
        }
        Some(o) => {
            let mut m = wallpaper_map::load();
            m.insert(o.clone(), path.to_string_lossy().to_string());
            wallpaper_map::save(&m);
        }
    }
}

/// Apply the wallpaper to desktop + lock screen. Writes the
/// picture URIs — ragnar-wallpaper watches these keys and the
/// outputs map file and respawns swaybg itself (single owner).
fn set_wallpaper(path: &Path) {
    let uri = format!("file://{}", path.to_string_lossy());
    let bg = gtk4::gio::Settings::new("org.gnome.desktop.background");
    let _ = bg.set_string("picture-uri", &uri);
    let _ = bg.set_string("picture-uri-dark", &uri);
    // Lock screen reads the screensaver picture-uri (screenlock).
    let saver = gtk4::gio::Settings::new("org.gnome.desktop.screensaver");
    let _ = saver.set_string("picture-uri", &uri);
}
