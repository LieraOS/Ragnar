//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! One editor tab: scrolled GtkTextView, undo/redo operation stack
//! (GTK4's TextBuffer has no built-in undo), dirty tracking and a
//! syntax highlighter applied to the whole buffer.

use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// One recorded buffer mutation, enough to undo and redo it.
#[derive(Clone)]
struct Op {
    offset: i32,
    text: String,
    inserted: bool,
}

pub struct Editor {
    pub scrolled: gtk4::ScrolledWindow,
    pub view: gtk4::TextView,
    pub path: RefCell<Option<PathBuf>>,
    pub dirty: Rc<RefCell<bool>>,
    undo: RefCell<Vec<Op>>,
    redo: RefCell<Vec<Op>>,
    /// Guards against recording ops triggered by undo/redo itself.
    applying: Rc<RefCell<bool>>,
    /// Coalesces consecutive single-direction edits into one step.
    last_inserted: RefCell<Option<bool>>,
    pub on_dirty: Rc<RefCell<Box<dyn Fn()>>>,
    pub on_cursor: Rc<RefCell<Box<dyn Fn()>>>,
    /// Fires once per file URI dropped onto the view.
    pub on_drop_file: Rc<RefCell<Box<dyn Fn(PathBuf)>>>,
}

pub fn new(font: &str, wrap: bool, line_numbers_visible: bool) -> Rc<Editor> {
    let view = gtk4::TextView::new();
    view.set_monospace(true);
    view.set_wrap_mode(if wrap {
        gtk4::WrapMode::Word
    } else {
        gtk4::WrapMode::None
    });
    view.set_left_margin(8);
    view.set_top_margin(6);
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&view));
    let _ = line_numbers_visible; // drawn by the gutter-less view; kept for API parity

    let ed = Rc::new(Editor {
        scrolled,
        view: view.clone(),
        path: RefCell::new(None),
        dirty: Rc::new(RefCell::new(false)),
        undo: RefCell::new(Vec::new()),
        redo: RefCell::new(Vec::new()),
        applying: Rc::new(RefCell::new(false)),
        last_inserted: RefCell::new(None),
        on_dirty: Rc::new(RefCell::new(Box::new(|| {}))),
        on_cursor: Rc::new(RefCell::new(Box::new(|| {}))),
        on_drop_file: Rc::new(RefCell::new(Box::new(|_| {}))),
    });

    view.add_css_class("ragnar-edit-view");
    let _ = font; // applied via CSS provider in main

    // Record insertions for the undo stack.
    {
        let ed2 = ed.clone();
        view.buffer().connect_insert_text(move |buf, pos, text| {
            if *ed2.applying.borrow() {
                return;
            }
            // Drag & drop of files inserts "file://…" lines through the
            // TextView's built-in handler — intercept them and open the
            // files instead of pasting the URI into the document.
            if text.len() > 8 {}
            if let Some(files) = uri_files(text) {
                glib::signal::signal_stop_emission_by_name(buf, "insert-text");
                for p in files {
                    (ed2.on_drop_file.borrow())(p);
                }
                return;
            }
            let offset = pos.offset();
            let ins = Some(true);
            if *ed2.last_inserted.borrow() == ins && !text.contains('\n') {
                if let Some(op) = ed2.undo.borrow_mut().last_mut() {
                    if op.inserted && op.offset + op.text.chars().count() as i32 == offset {
                        op.text.push_str(text);
                        ed2.redo.borrow_mut().clear();
                        mark_dirty(&ed2);
                        return;
                    }
                }
            }
            *ed2.last_inserted.borrow_mut() = ins;
            ed2.undo.borrow_mut().push(Op {
                offset,
                text: text.to_string(),
                inserted: true,
            });
            ed2.redo.borrow_mut().clear();
            let _ = buf;
            mark_dirty(&ed2);
        });
    }
    // Record deletions: grab the slice before it is removed.
    {
        let ed2 = ed.clone();
        view.buffer().connect_delete_range(move |buf, start, end| {
            if *ed2.applying.borrow() {
                return;
            }
            let txt = buf.text(start, end, true).to_string();
            *ed2.last_inserted.borrow_mut() = Some(false);
            ed2.undo.borrow_mut().push(Op {
                offset: start.offset(),
                text: txt,
                inserted: false,
            });
            ed2.redo.borrow_mut().clear();
            mark_dirty(&ed2);
        });
    }
    {
        let ed2 = ed.clone();
        view.buffer().connect_mark_set(move |_, _, mark| {
            if mark.name().as_deref() == Some("insert") {
                (ed2.on_cursor.borrow())();
            }
        });
    }
    // Claim file drops on the view itself — otherwise the TextView's
    // built-in handler wins and inserts the URI as literal text.
    {
        let cb = ed.on_drop_file.clone();
        let d = gtk4::DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );

        d.connect_drop(move |_, v, _, _| {
            if let Ok(fl) = v.get::<gtk4::gdk::FileList>() {
                for f in fl.files() {
                    if let Some(p) = f.path() {
                        (cb.borrow())(p);
                    }
                }
                true
            } else {
                false
            }
        });
        view.add_controller(d);
        // second FileList target on the scroll frame so drops on
        // margins/gutter area are caught too
        let cb2 = ed.on_drop_file.clone();
        let d2 = gtk4::DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );
        d2.connect_drop(move |_, v, _, _| {
            if let Ok(fl) = v.get::<gtk4::gdk::FileList>() {
                for f in fl.files() {
                    if let Some(p) = f.path() {
                        (cb2.borrow())(p);
                    }
                }
                true
            } else {
                false
            }
        });
        ed.scrolled.add_controller(d2);
    }
    // text/uri-list fallback for sources that don't provide FileList
    {
        let cb = ed.on_drop_file.clone();
        let d = gtk4::DropTarget::new(glib::types::Type::STRING, gtk4::gdk::DragAction::COPY);
        d.connect_drop(move |_, v, _, _| {
            let Ok(s) = v.get::<String>() else {
                return false;
            };
            if let Some(files) = uri_files(&s) {
                for p in files {
                    (cb.borrow())(p);
                }
                true
            } else {
                false
            }
        });
        view.add_controller(d);
    }
    ed
}

/// Returns file paths when `text` is a URI list (every non-empty line
/// starts with file://) — the shape the DnD machinery inserts.
fn uri_files(text: &str) -> Option<Vec<PathBuf>> {
    let mut v = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        // file managers send either file:// URIs or raw absolute paths
        let p = if let Some(rest) = l.strip_prefix("file://") {
            PathBuf::from(percent_decode(rest))
        } else if l.starts_with('/') {
            PathBuf::from(l)
        } else {
            return None; // not a file drop — plain text
        };
        if p.exists() {
            v.push(p);
        } else {
            return None; // looks like text mentioning paths, not a drop
        }
    }
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mark_dirty(ed: &Rc<Editor>) {
    if !*ed.dirty.borrow() {
        *ed.dirty.borrow_mut() = true;
        (ed.on_dirty.borrow())();
    }
}

pub fn set_text(ed: &Rc<Editor>, text: &str) {
    *ed.applying.borrow_mut() = true;
    ed.view.buffer().set_text(text);
    *ed.applying.borrow_mut() = false;
    ed.undo.borrow_mut().clear();
    ed.redo.borrow_mut().clear();
    *ed.dirty.borrow_mut() = false;
    *ed.last_inserted.borrow_mut() = None;
}

/// Reverts `undo`-top and pushes it on the redo stack.
pub fn undo(ed: &Rc<Editor>) {
    apply(ed, true);
}
pub fn redo(ed: &Rc<Editor>) {
    apply(ed, false);
}

fn apply(ed: &Rc<Editor>, is_undo: bool) {
    let op = if is_undo {
        ed.undo.borrow_mut().pop()
    } else {
        ed.redo.borrow_mut().pop()
    };
    let Some(op) = op else { return };
    *ed.applying.borrow_mut() = true;
    let buf = ed.view.buffer();
    let inverse: Op;
    if op.inserted {
        if is_undo {
            // delete the inserted range
            let mut a = buf.iter_at_offset(op.offset);
            let mut b = buf.iter_at_offset(op.offset + op.text.chars().count() as i32);
            buf.delete(&mut a, &mut b);
            inverse = Op {
                inserted: false,
                ..op
            };
        } else {
            let mut it = buf.iter_at_offset(op.offset);
            buf.insert(&mut it, &op.text);
            inverse = op;
        }
    } else if is_undo {
        let mut it = buf.iter_at_offset(op.offset);
        buf.insert(&mut it, &op.text);
        inverse = Op {
            inserted: true,
            ..op
        };
    } else {
        let mut a = buf.iter_at_offset(op.offset);
        let mut b = buf.iter_at_offset(op.offset + op.text.chars().count() as i32);
        buf.delete(&mut a, &mut b);
        inverse = op;
    }
    *ed.applying.borrow_mut() = false;
    if is_undo {
        ed.redo.borrow_mut().push(inverse);
    } else {
        ed.undo.borrow_mut().push(inverse);
    }
    *ed.last_inserted.borrow_mut() = None;
}

/// Cursor position as 1-based (line, column) for the status bar.
pub fn cursor_line_col(ed: &Rc<Editor>) -> (i32, i32) {
    let buf = ed.view.buffer();
    let it = buf.iter_at_mark(&buf.get_insert());
    (it.line() + 1, it.line_offset() + 1)
}
