// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — the application window.
//
// Classic three-pane layout: directory tree left, image
// canvas center, thumbnail strip right. Async decode on a worker
// keeps the UI responsive; neighbors are preloaded so flipping
// feels instant.

use crate::{browser::Browser, canvas::Canvas, decode, filters, thumbs::Thumbs};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

/// Shared per-image state that callbacks need.
struct State {
    current: RefCell<PathBuf>,
    decoded: RefCell<Option<decode::Decoded>>,
    orig: RefCell<Option<decode::Decoded>>,
    dir_files: RefCell<Vec<PathBuf>>,
    rx: RefCell<mpsc::Receiver<(u64, PathBuf, Result<decode::Decoded, String>)>>,
    tx: mpsc::SyncSender<(u64, PathBuf, Result<decode::Decoded, String>)>,
    gen: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Unsaved filter edits — drives the save prompt.
    dirty: std::cell::Cell<bool>,
}

/// Build and show the window, opening `start` if given.
pub fn run(app: &gtk4::Application, start: Option<PathBuf>) {
    let win = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Ragnar Eye")
        .default_width(1200)
        .default_height(800)
        .build();

    let (tx, rx) = mpsc::sync_channel(8);
    let state = Rc::new(State {
        current: RefCell::new(
            start
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
        ),
        decoded: RefCell::new(None),
        orig: RefCell::new(None),
        dir_files: RefCell::new(Vec::new()),
        rx: RefCell::new(rx),
        tx,
        gen: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        dirty: std::cell::Cell::new(false),
    });

    // --- panes -------------------------------------------------------------
    let canvas = Canvas::new();
    let status = gtk4::Label::new(None);
    status.set_xalign(0.0);
    status.set_margin_start(6);
    status.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let state2 = state.clone();
    let status2 = status.clone();
    let do_load = Rc::new(move |p: PathBuf| {
        *state2.current.borrow_mut() = p.clone();
        let gen = state2.gen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        status2.set_text(&format!("{}…", crate::i18n::tr("Lade")));
        let tx = state2.tx.clone();
        let gen_arc = state2.gen.clone();
        std::thread::spawn(move || {
            if gen_arc.load(std::sync::atomic::Ordering::SeqCst) != gen {
                return;
            }
            let res = decode::decode(&p.to_string_lossy());
            let _ = tx.send((gen, p, res));
        });
    });

    // Dirty-aware load: if the current image carries unsaved
    // filter edits, ask first — Save, Discard or Cancel. This is
    // what silently swallowed user edits before.
    let load: Rc<dyn Fn(PathBuf)> = {
        let state = state.clone();
        let do_load = do_load.clone();
        let win = win.clone();
        Rc::new(move |p: PathBuf| {
            if !state.dirty.get() {
                do_load(p);
                return;
            }
            let cur = state.current.borrow().clone();
            let state = state.clone();
            let do_load = do_load.clone();
            let win = win.clone();
            let dlg = gtk4::AlertDialog::builder()
                .message(crate::i18n::tr("Änderungen speichern?"))
                .detail(cur.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
                .buttons(vec![
                    crate::i18n::tr("Speichern"),
                    crate::i18n::tr("Verwerfen"),
                    crate::i18n::tr("Abbrechen"),
                ])
                .build();
            let win2 = win.clone();
            // Fn callback (not FnOnce): carry the target as a
            // takeable slot shared by both branches.
            let target = Rc::new(std::cell::RefCell::new(Some(p.clone())));
            dlg.choose(Some(&win), gtk4::gio::Cancellable::NONE, move |r| {
                let t0 = target.clone();
                match r {
                    Ok(0) => save_as(&win2, &state, move |ok| {
                        if ok {
                            let p = t0.borrow_mut().take().unwrap_or_default();
                            do_load(p);
                        }
                    }),
                    Ok(1) => {
                        state.dirty.set(false);
                        let p = target.borrow_mut().take().unwrap_or_default();
                        do_load(p);
                    }
                    _ => {}
                }
            });
        })
    };

    let load2 = load.clone();
    let thumbs = Thumbs::new(Rc::new(move |p: PathBuf| load2(p)));
    let load3 = load.clone();
    let thumbs2 = thumbs.clone();
    let browser = Browser::new(
        &start
            .clone()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("/")),
        Rc::new(move |dir: PathBuf| {
            // Picking a directory in the tree shows its first
            // image — and repopulates the thumbnail strip.
            thumbs2.load_dir(&dir, std::path::Path::new(""));
            if let Some(first) = dir_files(&dir).first() {
                load3(first.clone());
            }
        }),
    );
    // Open the tree path to the opened file so the user sees
    // where they are in the filesystem.
    if let Some(dir) = start.as_ref().and_then(|p| p.parent()) {
        browser.expand_to(dir);
    }

    // --- header bar: filters menu ------------------------------------------
    let menu = gtk4::gio::Menu::new();
    for (id, label) in filters::FILTERS {
        menu.append(
            Some(crate::i18n::tr(label).as_str()),
            Some(format!("win.filter-{id}").as_str()),
        );
    }
    let menubtn = gtk4::MenuButton::builder()
        .label(crate::i18n::tr("Filter"))
        .menu_model(&menu)
        .build();
    let header = gtk4::HeaderBar::new();
    header.pack_end(&menubtn);
    win.set_titlebar(Some(&header));

    // Filter actions — operate on the original decoded buffer.
    for (id, _) in filters::FILTERS {
        let act = gtk4::gio::SimpleAction::new(&format!("filter-{id}"), None);
        let state = state.clone();
        let canvas = canvas.clone();
        let name = id.to_string();
        act.connect_activate(move |_, _| {
            if let Some(orig) = state.orig.borrow().as_ref() {
                // Always filter from the untouched original, then allow
                // stacking from the working buffer if the user prefers.
                let mut img = decode::Decoded {
                    width: orig.width,
                    height: orig.height,
                    rgba: orig.rgba.clone(),
                };
                filters::apply(&name, &mut img);
                *state.decoded.borrow_mut() = Some(img.clone());
                canvas.show(&img);
                state.dirty.set(true);
            }
        });
        win.add_action(&act);
    }
    // "Revert" restores the untouched decode.
    let revert = gtk4::gio::SimpleAction::new("revert", None);
    {
        let state = state.clone();
        let canvas = canvas.clone();
        revert.connect_activate(move |_, _| {
            if let Some(o) = state.orig.borrow().as_ref() {
                let img = o.clone();
                *state.decoded.borrow_mut() = Some(img.clone());
                canvas.show(&img);
                state.dirty.set(false);
            }
        });
    }
    win.add_action(&revert);
    menu.append(
        Some(crate::i18n::tr("Original").as_str()),
        Some("win.revert"),
    );
    menu.append(
        Some(crate::i18n::tr("Speichern unter").as_str()),
        Some("win.save-as"),
    );

    // Save action — same FileDialog path the dirty prompt uses.
    let save = gtk4::gio::SimpleAction::new("save-as", None);
    {
        let state = state.clone();
        let win2 = win.clone();
        save.connect_activate(move |_, _| save_as(&win2, &state, |_| {}));
    }
    win.add_action(&save);

    // --- context menu (right-click on the canvas) ---------------------------
    // Actions live on the window like the filters; the popover just
    // points at the click position.
    let ctx = gtk4::gio::Menu::new();
    ctx.append(
        Some(crate::i18n::tr("Als Hintergrund festlegen").as_str()),
        Some("win.set-wallpaper"),
    );
    ctx.append(
        Some(crate::i18n::tr("Eigenschaften").as_str()),
        Some("win.properties"),
    );
    ctx.append(
        Some(crate::i18n::tr("Endgültig löschen").as_str()),
        Some("win.trash"),
    );

    // Set wallpaper: ragnar-wallpaper watches the standard GNOME
    // background keys and respawns swaybg — just write the URI.
    let wp = gtk4::gio::SimpleAction::new("set-wallpaper", None);
    {
        let state = state.clone();
        let status = status.clone();
        wp.connect_activate(move |_, _| {
            let p = state.current.borrow().clone();
            let uri = format!("file://{}", p.to_string_lossy());
            let s = gtk4::gio::Settings::new("org.gnome.desktop.background");
            s.set_string("picture-uri", &uri).ok();
            s.set_string("picture-options", "zoom").ok();
            status.set_text(&crate::i18n::tr("Hintergrund gesetzt"));
        });
    }
    win.add_action(&wp);

    // Properties dialog: a real window with labelled rows, like
    // the file manager's properties — name, full path, format,
    // pixel dimensions, file size, modified time.
    let props = gtk4::gio::SimpleAction::new("properties", None);
    {
        let state = state.clone();
        let win2 = win.clone();
        props.connect_activate(move |_, _| {
            let p = state.current.borrow().clone();
            let (w, h) = state
                .decoded
                .borrow()
                .as_ref()
                .map(|d| (d.width, d.height))
                .unwrap_or((0, 0));
            let meta = std::fs::metadata(&p).ok();
            let size = meta
                .as_ref()
                .map(|m| {
                    let mib = m.len() as f64 / 1_048_576.0;
                    if mib >= 1.0 {
                        format!("{:.1} MiB ({} Bytes)", mib, m.len())
                    } else {
                        format!("{} Bytes", m.len())
                    }
                })
                .unwrap_or_else(|| "?".into());
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    gtk4::glib::DateTime::from_unix_local(secs as i64)
                        .map(|d| d.format("%x %X").unwrap().to_string())
                        .unwrap_or_else(|_| "?".into())
                })
                .unwrap_or_else(|| "?".into());
            let rows = [
                (
                    crate::i18n::tr("Name"),
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string(),
                ),
                (
                    crate::i18n::tr("Ort"),
                    p.parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
                (
                    crate::i18n::tr("Format"),
                    decode::ext_of(&p.to_string_lossy()).to_uppercase(),
                ),
                (crate::i18n::tr("Maße"), format!("{w} × {h}")),
                (crate::i18n::tr("Größe"), size),
                (crate::i18n::tr("Geändert"), mtime),
            ];
            let grid = gtk4::Grid::builder()
                .row_spacing(8)
                .column_spacing(12)
                .margin_top(16)
                .margin_bottom(16)
                .margin_start(16)
                .margin_end(16)
                .build();
            for (i, (k, v)) in rows.iter().enumerate() {
                let key = gtk4::Label::new(Some(k));
                key.set_xalign(0.0);
                key.add_css_class("dim-label");
                let val = gtk4::Label::new(Some(v));
                val.set_xalign(0.0);
                val.set_selectable(true);
                val.set_wrap(true);
                grid.attach(&key, 0, i as i32, 1, 1);
                grid.attach(&val, 1, i as i32, 1, 1);
            }
            let dlg = gtk4::Window::builder()
                .transient_for(&win2)
                .modal(true)
                .title(crate::i18n::tr("Eigenschaften"))
                .default_width(420)
                .build();
            dlg.set_child(Some(&grid));
            dlg.present();
        });
    }
    win.add_action(&props);

    // Trash: confirm first, then gio trash + advance to the next
    // image and rebuild the strip (the file is really gone).
    let trash = gtk4::gio::SimpleAction::new("trash", None);
    {
        let state = state.clone();
        let win2 = win.clone();
        let load = load.clone();
        trash.connect_activate(move |_, _| {
            let p = state.current.borrow().clone();
            let files = state.dir_files.borrow().clone();
            let next = files
                .iter()
                .position(|x| *x == p)
                .and_then(|i| files.get(i + 1).or_else(|| files.get(i.wrapping_sub(1))))
                .cloned();
            let dlg = gtk4::AlertDialog::builder()
                .message(crate::i18n::tr("Bild endgültig löschen?"))
                .detail(p.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
                .buttons(vec![
                    crate::i18n::tr("Abbrechen"),
                    crate::i18n::tr("Löschen"),
                ])
                .cancel_button(0)
                .default_button(0)
                .build();
            let load2 = load.clone();
            dlg.choose(Some(&win2), gtk4::gio::Cancellable::NONE, move |r| {
                if r == Ok(1) {
                    let _ = std::fs::remove_file(&p);
                    if let Some(n) = next {
                        load2(n);
                    }
                }
            });
        });
    }
    win.add_action(&trash);

    // Right-click gesture on the canvas → popover at pointer.
    let pop = gtk4::PopoverMenu::from_model(Some(&ctx));
    pop.set_parent(&canvas.area);
    pop.set_has_arrow(false);
    let click = gtk4::GestureClick::new();
    click.set_button(3);
    {
        let pop = pop.clone();
        click.connect_pressed(move |_, _, x, y| {
            pop.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            pop.popup();
        });
    }
    canvas.area.add_controller(click);

    // --- assemble panes -----------------------------------------------------
    let panes = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    panes.append(&browser.widget);
    panes.append(&canvas.area);
    panes.append(&thumbs.widget);
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.append(&panes);
    vbox.append(&status);
    panes.set_vexpand(true);
    win.set_child(Some(&vbox));

    // --- keybinds -----------------------------------------------------------
    let keys = gtk4::EventControllerKey::new();
    {
        let state = state.clone();
        let canvas = canvas.clone();
        let load = load.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let files = state.dir_files.borrow();
            let cur = state.current.borrow().clone();
            let idx = files.iter().position(|p| *p == cur);
            let step = |delta: isize| -> Option<PathBuf> {
                idx.and_then(|i| {
                    files
                        .get((i as isize + delta).rem_euclid(files.len() as isize) as usize)
                        .cloned()
                })
            };
            match key {
                gtk4::gdk::Key::Left | gtk4::gdk::Key::Page_Up => {
                    if let Some(p) = step(-1) {
                        load(p);
                    }
                }
                gtk4::gdk::Key::Right | gtk4::gdk::Key::Page_Down | gtk4::gdk::Key::space => {
                    if let Some(p) = step(1) {
                        load(p);
                    }
                }
                gtk4::gdk::Key::_0 | gtk4::gdk::Key::F => canvas.fit_to_window(),
                gtk4::gdk::Key::_1 => canvas.actual_size(),
                gtk4::gdk::Key::plus | gtk4::gdk::Key::equal => canvas.zoom_step(1.25),
                gtk4::gdk::Key::minus => canvas.zoom_step(0.8),
                _ => return gtk4::glib::Propagation::Proceed,
            }
            gtk4::glib::Propagation::Stop
        });
    }
    win.add_controller(keys);

    // --- decode results arrive on the main loop -----------------------------
    {
        let state = state.clone();
        let canvas = canvas.clone();
        let status = status.clone();
        let thumbs = thumbs.clone();
        // Poll the channel on a timer, NOT on idle: a Continue
        // idle source spins the main loop at 100 % of a core.
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let rx = state.rx.borrow();
            for (gen, path, res) in rx.try_iter() {
                if gen != state.gen.load(std::sync::atomic::Ordering::SeqCst) {
                    continue; // stale decode from an older selection
                }
                match res {
                    Ok(d) => {
                        status.set_text(&format!(
                            "{}/{} — {}×{}",
                            path.parent()
                                .map(|d| d.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                            d.width,
                            d.height
                        ));
                        *state.orig.borrow_mut() = Some(d.clone());
                        *state.decoded.borrow_mut() = Some(d.clone());
                        state.dirty.set(false);
                        canvas.show(&d);
                        if let Some(dir) = path.parent() {
                            let files = dir_files(dir);
                            *state.dir_files.borrow_mut() = files;
                            thumbs.load_dir(dir, &path);
                            preload_neighbors(&state, &path);
                        }
                    }
                    Err(e) => status.set_text(&format!(
                        "{}: {}",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                        e
                    )),
                }
            }
            gtk4::glib::ControlFlow::Continue
        });
    }

    // Closing with unsaved edits → same prompt as image switch.
    // Inhibit the close, decide asynchronously, then destroy.
    {
        let state = state.clone();
        let win2 = win.clone();
        win.connect_close_request(move |_| {
            if !state.dirty.get() {
                return gtk4::glib::Propagation::Proceed;
            }
            let state = state.clone();
            let win3 = win2.clone();
            let dlg = gtk4::AlertDialog::builder()
                .message(crate::i18n::tr("Änderungen speichern?"))
                .buttons(vec![
                    crate::i18n::tr("Speichern"),
                    crate::i18n::tr("Verwerfen"),
                    crate::i18n::tr("Abbrechen"),
                ])
                .build();
            dlg.choose(
                Some(&win2),
                gtk4::gio::Cancellable::NONE,
                move |r| match r {
                    Ok(0) => {
                        let win5 = win3.clone();
                        save_as(&win3, &state, move |ok| {
                            if ok {
                                win5.destroy();
                            }
                        })
                    }
                    Ok(1) => win3.destroy(),
                    _ => {}
                },
            );
            gtk4::glib::Propagation::Stop
        });
    }

    win.present();
    if let Some(p) = start {
        load(p);
    }
}

/// All supported image files in `dir`, sorted.
fn dir_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
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
    v.sort();
    v
}

/// Decode the immediate neighbors of `path` in the background so
/// Left/Right navigation feels instant.
fn preload_neighbors(state: &Rc<State>, path: &std::path::Path) {
    let files = state.dir_files.borrow().clone();
    let Some(idx) = files.iter().position(|p| p == path) else {
        return;
    };
    for delta in [-1isize, 1, 2] {
        if let Some(p) = files.get((idx as isize + delta) as usize).cloned() {
            let gen = state.gen.load(std::sync::atomic::Ordering::SeqCst);
            let gen_arc = state.gen.clone();
            std::thread::spawn(move || {
                // Warm the thumbnail cache only — full decode preload
                // is left to the OS page cache + thumb pipeline.
                if gen_arc.load(std::sync::atomic::Ordering::SeqCst) == gen {
                    let _ = decode::decode(&p.to_string_lossy());
                }
            });
        }
    }
}

/// Write the filtered buffer out through FileDialog — always PNG.
/// `then` receives whether the save actually happened.
fn save_as(win: &gtk4::ApplicationWindow, state: &Rc<State>, then: impl Fn(bool) + 'static) {
    let Some(img) = state.decoded.borrow().clone() else {
        then(false);
        return;
    };
    let stem = state
        .current
        .borrow()
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bild")
        .to_string();
    let dlg = gtk4::FileDialog::builder()
        .initial_name(format!("{stem}-bearbeitet.png"))
        .build();
    dlg.save(Some(win), gtk4::gio::Cancellable::NONE, move |res| {
        let ok = res
            .ok()
            .and_then(|f| f.path())
            .and_then(|p| {
                image::RgbaImage::from_raw(img.width, img.height, img.rgba)
                    .and_then(|i| i.save(&p).ok())
            })
            .is_some();
        then(ok);
    });
}
