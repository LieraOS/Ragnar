// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — the right-hand thumbnail strip.
//
// Lists every supported image in the current directory as a
// scrollable column of thumbnails. Decodes happen on worker
// threads (scaled to ~128px), cached on disk via cache.rs, and
// pushed back to the GTK main loop through an mpsc channel.
// Clicking a thumbnail asks the window to show that file.

use crate::{cache, decode};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

pub const THUMB_PX: u32 = 128;

/// A finished thumbnail ready for the UI thread.
struct ThumbResult {
    path: PathBuf,
    tex: Option<gtk4::gdk::Texture>,
}

/// The strip widget + its communication endpoints.
pub struct Thumbs {
    pub widget: gtk4::ScrolledWindow,
    list: gtk4::ListBox,
    rx: RefCell<mpsc::Receiver<ThumbResult>>,
    tx: mpsc::SyncSender<ThumbResult>,
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Directory currently displayed — rebuilt only when it
    /// changes, so the scroll position survives image switches.
    current_dir: RefCell<Option<PathBuf>>,
}

impl Thumbs {
    /// `on_pick` is invoked with the file path when a thumbnail is
    /// activated.
    pub fn new(on_pick: Rc<dyn Fn(PathBuf)>) -> Rc<Thumbs> {
        let (tx, rx) = mpsc::sync_channel(64);
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::Single);
        list.add_css_class("navigation-sidebar");

        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(THUMB_PX as i32 + 20)
            .build();
        widget.set_child(Some(&list));

        let t = Rc::new(Thumbs {
            widget,
            list: list.clone(),
            rx: RefCell::new(rx),
            tx,
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            current_dir: RefCell::new(None),
        });

        // Drain finished thumbs on the main loop.
        {
            let t = t.clone();
            gtk4::glib::idle_add_local(move || {
                let rx = t.rx.borrow();
                for r in rx.try_iter() {
                    // Row lookup by stored path.
                    let mut row = t.list.first_child();
                    while let Some(w) = row {
                        if let Ok(lb) = w.clone().downcast::<gtk4::ListBoxRow>() {
                            let stored: Option<PathBuf> =
                                unsafe { lb.data::<PathBuf>("path").map(|p| p.as_ref().clone()) };
                            if stored.as_deref() == Some(r.path.as_path()) {
                                if let Some(tex) = &r.tex {
                                    let pic = gtk4::Picture::for_paintable(tex);
                                    pic.set_size_request(THUMB_PX as i32, THUMB_PX as i32);
                                    lb.set_child(Some(&pic));
                                }
                            }
                        }
                        row = w.next_sibling();
                    }
                }
                gtk4::glib::ControlFlow::Continue
            });
        }

        // Activation → ask the window to load the file.
        list.connect_row_activated(move |_, row| {
            let p: Option<PathBuf> =
                unsafe { row.data::<PathBuf>("path").map(|p| p.as_ref().clone()) };
            if let Some(p) = p {
                on_pick(p);
            }
        });
        t
    }

    /// Repopulate the strip for `dir`, highlighting `current`.
    /// If `dir` is already displayed this only updates the
    /// selection — the rows (and the scroll position) stay put,
    /// which is what made clicking a thumbnail feel like a reload.
    pub fn load_dir(&self, dir: &std::path::Path, current: &std::path::Path) {
        if self.current_dir.borrow().as_deref() == Some(dir) {
            self.select(current);
            return;
        }
        *self.current_dir.borrow_mut() = Some(dir.to_path_buf());
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let gen = self.generation.load(std::sync::atomic::Ordering::SeqCst);
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file()
                            && decode::supported_extensions()
                                .contains(&decode::ext_of(&p.to_string_lossy()).as_str())
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();

        for p in &files {
            let row = gtk4::ListBoxRow::new();
            // Placeholder label while the thumb decodes.
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            let lbl = gtk4::Label::new(Some(&name));
            lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row.set_child(Some(&lbl));
            unsafe { row.set_data("path", p.clone()) };
            self.list.append(&row);
            if p == current {
                self.list.select_row(Some(&row));
            }
        }

        // ONE sequential worker for the whole directory: a thread
        // per file meant N full-size decodes hammering every core
        // at once — the startup storm the user felt. The selected
        // image is generated first so the strip fills top-down.
        let tx = self.tx.clone();
        let gen_arc = self.generation.clone();
        let current = current.to_path_buf();
        std::thread::spawn(move || {
            let mut ordered = files;
            if let Some(i) = ordered.iter().position(|p| *p == current) {
                ordered.swap(0, i);
            }
            for p in ordered {
                if gen_arc.load(std::sync::atomic::Ordering::SeqCst) != gen {
                    return;
                }
                let tex = make_thumb(&p);
                if tx.send(ThumbResult { path: p, tex }).is_err() {
                    return;
                }
            }
        });
    }

    /// Highlight the row for `path` without touching the list —
    /// used when switching images inside the same directory.
    pub fn select(&self, path: &std::path::Path) {
        let mut row = self.list.first_child();
        while let Some(w) = row {
            if let Ok(lb) = w.clone().downcast::<gtk4::ListBoxRow>() {
                let stored: Option<PathBuf> =
                    unsafe { lb.data::<PathBuf>("path").map(|p| p.as_ref().clone()) };
                if stored.as_deref() == Some(path) {
                    self.list.select_row(Some(&lb));
                    return;
                }
            }
            row = w.next_sibling();
        }
        self.list.unselect_all();
    }
}

/// Produce a thumbnail texture for `path`: disk cache first,
/// then a full decode + box downscale.
fn make_thumb(path: &std::path::Path) -> Option<gtk4::gdk::Texture> {
    let ps = path.to_string_lossy().to_string();
    if let Some(png_path) = cache::get(&ps) {
        if let Ok(tex) = gtk4::gdk::Texture::from_file(&gtk4::gio::File::for_path(&png_path)) {
            return Some(tex);
        }
    }
    let d = decode::decode(&ps).ok()?;
    // Downscale to fit THUMB_PX preserving aspect.
    let scale = (THUMB_PX as f64 / d.width as f64)
        .min(THUMB_PX as f64 / d.height as f64)
        .min(1.0);
    let (tw, th) = (
        (d.width as f64 * scale) as u32,
        (d.height as f64 * scale) as u32,
    );
    let mut thumb = vec![0u8; (tw * th) as usize * 4];
    for y in 0..th {
        for x in 0..tw {
            let sx = (x as f64 / scale) as u32;
            let sy = (y as f64 / scale) as u32;
            let s = ((sy * d.width + sx) * 4) as usize;
            let t = ((y * tw + x) * 4) as usize;
            thumb[t..t + 4].copy_from_slice(&d.rgba[s..s + 4]);
        }
    }
    // Persist the thumbnail as PNG for next time.
    let mut buf = std::io::Cursor::new(Vec::new());
    use image::ImageEncoder;
    if image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(&thumb, tw, th, image::ExtendedColorType::Rgba8)
        .is_ok()
    {
        cache::put(&ps, &buf.into_inner());
    }
    let bytes = gtk4::glib::Bytes::from_owned(thumb);
    Some(
        gtk4::gdk::MemoryTexture::new(
            tw as i32,
            th as i32,
            gtk4::gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            (tw * 4) as usize,
        )
        .upcast(),
    )
}
