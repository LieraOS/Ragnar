//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Window actions, accelerators, file open/save plumbing and zoom.

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::editor;
use crate::fileio;
use crate::highlight;
use crate::install_font_css;
use crate::print;
use crate::search;
use crate::settings::Settings;
use crate::settingsdlg;
use crate::tabs::{self, Tabs};

pub fn save_editor(ed: &Rc<editor::Editor>, path: &std::path::Path) -> std::io::Result<()> {
    let buf = ed.view.buffer();
    let (s, e) = buf.bounds();
    std::fs::write(path, buf.text(&s, &e, true).as_bytes())
}

pub fn wire_actions(
    w: &adw::ApplicationWindow,
    app: &adw::Application,
    tabs: &Rc<Tabs>,
    settings: &Rc<RefCell<Settings>>,
    new_tab: &Rc<dyn Fn() -> adw::TabPage>,
    set_status: &Rc<dyn Fn(String)>,
) {
    let add = |name: &str, accels: &[&str], f: Rc<dyn Fn()>| {
        let a = gio::SimpleAction::new(name, None);
        a.connect_activate(move |_, _| f());
        w.add_action(&a);
        if !accels.is_empty() {
            app.set_accels_for_action(&format!("win.{name}"), accels);
        }
    };
    let (t, st, nt) = (tabs.clone(), set_status.clone(), new_tab.clone());
    add(
        "new",
        &["<Ctrl>n"],
        Rc::new(move || {
            nt();
            let _ = &t;
            st(gettext("New document"));
        }),
    );
    let (t, w2, st) = (tabs.clone(), w.clone(), set_status.clone());
    add(
        "open",
        &["<Ctrl>o"],
        Rc::new(move || open_file(&w2, &t, st.clone())),
    );
    let (t, w2, st) = (tabs.clone(), w.clone(), set_status.clone());
    add(
        "save",
        &["<Ctrl>s"],
        Rc::new(move || save(&w2, &t, false, st.clone())),
    );
    let (t, w2, st) = (tabs.clone(), w.clone(), set_status.clone());
    add(
        "save-as",
        &["<Ctrl><Shift>s"],
        Rc::new(move || save(&w2, &t, true, st.clone())),
    );
    let (t, w2) = (tabs.clone(), w.clone());
    add(
        "print",
        &["<Ctrl>p"],
        Rc::new(move || {
            if let Some((ed, page)) = tabs::current(&t) {
                let buf = ed.view.buffer();
                let (s, e) = buf.bounds();
                print::run(
                    &w2.clone().upcast(),
                    buf.text(&s, &e, true).to_string(),
                    page.title().as_str(),
                );
            }
        }),
    );
    let w2 = w.clone();
    add("quit", &["<Ctrl>q"], Rc::new(move || w2.close()));
    let t = tabs.clone();
    add(
        "undo",
        &["<Ctrl>z"],
        Rc::new(move || {
            if let Some((e, _)) = tabs::current(&t) {
                editor::undo(&e);
            }
        }),
    );
    let t = tabs.clone();
    add(
        "redo",
        &["<Ctrl>y"],
        Rc::new(move || {
            if let Some((e, _)) = tabs::current(&t) {
                editor::redo(&e);
            }
        }),
    );
    let t = tabs.clone();
    add(
        "cut",
        &["<Ctrl>x"],
        Rc::new(move || {
            if let Some((e, _)) = tabs::current(&t) {
                e.view.clipboard();
                let b = e.view.buffer();
                b.cut_clipboard(&e.view.clipboard(), e.view.is_editable());
            }
        }),
    );
    let t = tabs.clone();
    add(
        "copy",
        &["<Ctrl>c"],
        Rc::new(move || {
            if let Some((e, _)) = tabs::current(&t) {
                e.view.buffer().copy_clipboard(&e.view.clipboard());
            }
        }),
    );
    let t = tabs.clone();
    add(
        "paste",
        &["<Ctrl>v"],
        Rc::new(move || {
            if let Some((e, _)) = tabs::current(&t) {
                e.view
                    .buffer()
                    .paste_clipboard(&e.view.clipboard(), None, e.view.is_editable());
            }
        }),
    );
    let (t, w2, st) = (tabs.clone(), w.clone(), set_status.clone());
    add(
        "find",
        &["<Ctrl>h"],
        Rc::new(move || search::open(&w2.clone().upcast(), t.clone(), st.clone())),
    );
    let s = settings.clone();
    add("zoom-in", &["<Ctrl>plus"], Rc::new(move || zoom(&s, 0.1)));
    let s = settings.clone();
    add(
        "zoom-out",
        &["<Ctrl>minus"],
        Rc::new(move || zoom(&s, -0.1)),
    );
    let s = settings.clone();
    add(
        "zoom-reset",
        &["<Ctrl>0"],
        Rc::new(move || {
            install_font_css(&s.borrow().font, 1.0);
        }),
    );
    let (w2, s, t2) = (w.clone(), settings.clone(), tabs.clone());
    add(
        "settings",
        &[],
        Rc::new(move || {
            let apply = apply_settings_cb(&t2, &s);
            settingsdlg::open(w2.upcast_ref(), s.clone(), apply);
        }),
    );
    let w2 = w.clone();
    add(
        "about",
        &[],
        Rc::new(move || {
            let a = adw::AboutDialog::new();
            a.set_application_name("Ragnar Edit");
            a.set_version("0.1.0");
            a.set_developer_name("Jörg Schröder");
            a.set_license_type(gtk4::License::Gpl30);
            a.set_website("https://www.lieraos.com");
            a.set_comments(&gettext(
                "Native Wayland text editor for the Ragnar desktop",
            ));
            a.present(Some(w2.upcast_ref::<gtk4::Window>()));
        }),
    );
}

/// Applies theme/font/wrap live to every open tab — the callback the
/// settings dialog invokes after each change.
fn apply_settings_cb(tabs: &Rc<Tabs>, s: &Rc<RefCell<Settings>>) -> Rc<dyn Fn()> {
    let (t, st) = (tabs.clone(), s.clone());
    Rc::new(move || {
        let cfg = st.borrow();
        adw::StyleManager::default().set_color_scheme(if cfg.dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        crate::install_font_css(&cfg.font, 1.0);
        for (ed, _) in tabs::all(&t) {
            highlight::retint(&ed.view.buffer());
            ed.view.set_wrap_mode(if cfg.word_wrap {
                gtk4::WrapMode::Word
            } else {
                gtk4::WrapMode::None
            });
        }
    })
}

pub fn zoom(s: &Rc<RefCell<Settings>>, d: f64) {
    // Zoom is session-local: nudge the font size entry, don't persist.
    let font = s.borrow().font.clone();
    let (fam, sz) = font
        .rsplit_once(' ')
        .map(|(f, x)| (f.to_string(), x.parse::<f64>().unwrap_or(11.0)))
        .unwrap_or((font, 11.0));
    install_font_css(
        &format!("{} {}", fam, (sz * (1.0 + d)).clamp(6.0, 72.0)),
        1.0,
    );
}

pub fn open_file(w: &adw::ApplicationWindow, tabs: &Rc<Tabs>, set_status: Rc<dyn Fn(String)>) {
    let dlg = gtk4::FileDialog::new();
    let (t, w2) = (tabs.clone(), w.clone());
    dlg.open(
        Some(&w2.clone().upcast::<gtk4::Window>()),
        None::<&gio::Cancellable>,
        move |r| {
            if let Ok(f) = r {
                if let Some(p) = f.path() {
                    load_into_tab(&t, p, set_status.clone());
                }
            }
        },
    );
}

/// Loads a file on a worker thread, then fills a new tab on the
/// main loop — chunked IO keeps multi-MB opens non-blocking.
pub fn load_into_tab(tabs: &Rc<Tabs>, path: PathBuf, set_status: Rc<dyn Fn(String)>) {
    if fileio::is_binary(&path) {
        set_status(gettext("Binary file — not opened"));
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<fileio::Loaded>>();
    let p2 = path.clone();
    std::thread::spawn(move || {
        let _ = tx.send(fileio::load(&p2));
    });
    let t = tabs.clone();
    glib::timeout_add_local(Duration::from_millis(60), move || match rx.try_recv() {
        Ok(Ok(loaded)) => {
            // reuse an untouched current tab, else append
            let (ed, page) = match tabs::current(&t) {
                Some((e, p))
                    if !*e.dirty.borrow()
                        && e.path.borrow().is_none()
                        && e.view.buffer().char_count() == 0 =>
                {
                    (e, p)
                }
                _ => {
                    // fresh tab for each further dropped file
                    let (e, p) = tabs::add(&t, "Monospace 11", true, true);
                    *e.on_cursor.borrow_mut() = Box::new(|| {});
                    let pc = p.clone();
                    *e.on_dirty.borrow_mut() =
                        Box::new(move || tabs::set_dirty_indicator(&pc, true));
                    let (tt, ss) = (t.clone(), set_status.clone());
                    *e.on_drop_file.borrow_mut() =
                        Box::new(move |pb| load_into_tab(&tt, pb, ss.clone()));
                    (e, p)
                }
            };
            editor::set_text(&ed, &loaded.text);
            *ed.path.borrow_mut() = Some(path.clone());
            tabs::retitle(&page, &path);
            ed.view.set_editable(!loaded.readonly);
            let lang = path
                .extension()
                .and_then(|e| e.to_str())
                .map(highlight::from_ext)
                .unwrap_or(highlight::Lang::Plain);
            highlight::apply(&ed.view.buffer(), lang);
            set_status(format!("{} {}", gettext("Opened"), path.display()));
            glib::ControlFlow::Break
        }
        Ok(Err(_)) => {
            set_status(gettext("Could not open file"));
            glib::ControlFlow::Break
        }
        Err(_) => glib::ControlFlow::Continue,
    });
}

pub fn save(
    w: &adw::ApplicationWindow,
    tabs: &Rc<Tabs>,
    ask: bool,
    set_status: Rc<dyn Fn(String)>,
) {
    let Some((ed, page)) = tabs::current(tabs) else {
        return;
    };
    if let Some(p) = ed.path.borrow().clone() {
        if !ask && save_editor(&ed, &p).is_ok() {
            *ed.dirty.borrow_mut() = false;
            tabs::set_dirty_indicator(&page, false);
            set_status(gettext("File saved"));
            return;
        }
        if !ask {
            set_status(gettext("Could not save file"));
            return;
        }
    }
    let dlg = gtk4::FileDialog::new();
    let (t2, w2) = (tabs.clone(), w.clone());
    dlg.save(
        Some(&w2.clone().upcast::<gtk4::Window>()),
        None::<&gio::Cancellable>,
        move |r| {
            if let Ok(f) = r {
                if let Some(p) = f.path() {
                    if let Some((e, pg)) = tabs::current(&t2) {
                        if save_editor(&e, &p).is_ok() {
                            *e.dirty.borrow_mut() = false;
                            *e.path.borrow_mut() = Some(p.clone());
                            tabs::retitle(&pg, &p);
                            tabs::set_dirty_indicator(&pg, false);
                            set_status(gettext("File saved"));
                        }
                    }
                }
            }
        },
    );
}

/// Saves every dirty tab that has a path; pathless ones get
/// sequential save-as dialogs. A cancelled save-as aborts closing.
pub fn save_all_then_close(tabs: &Rc<tabs::Tabs>, w: &adw::ApplicationWindow) {
    let mut pending = Vec::new();
    for (ed, page) in tabs::all(tabs) {
        if !*ed.dirty.borrow() {
            continue;
        }
        let p = ed.path.borrow().clone();
        match p {
            Some(p) => {
                if save_editor(&ed, &p).is_ok() {
                    *ed.dirty.borrow_mut() = false;
                    tabs::set_dirty_indicator(&page, false);
                }
            }
            None => pending.push((ed, page)),
        }
    }
    if pending.is_empty() {
        w.destroy();
    } else {
        save_pending(w.upcast_ref(), pending);
    }
}

fn save_pending(w: &gtk4::Window, mut pending: Vec<(Rc<editor::Editor>, adw::TabPage)>) {
    let Some((ed, page)) = pending.pop() else {
        w.destroy();
        return;
    };
    let dlg = gtk4::FileDialog::new();
    let w2 = w.clone();
    dlg.save(Some(w), None::<&gio::Cancellable>, move |r| {
        if let Ok(f) = r {
            if let Some(p) = f.path() {
                if save_editor(&ed, &p).is_ok() {
                    *ed.dirty.borrow_mut() = false;
                    tabs::retitle(&page, &p);
                    tabs::set_dirty_indicator(&page, false);
                }
                save_pending(&w2, pending);
            }
        }
        // cancelled → abort the whole close, window stays open
    });
}
