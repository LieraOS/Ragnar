//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Find & Replace dialog: case sensitivity, whole-word toggle and an
//! option to search across all open tabs (the original's multi-tab
//! search). Highlighting uses a dedicated TextTag on every match.

use gtk4::prelude::*;
use std::rc::Rc;

use crate::editor;
use crate::tabs::{self, Tabs};

const TAG: &str = "findhit";

fn find_tag(buf: &gtk4::TextBuffer) -> gtk4::TextTag {
    if let Some(t) = buf.tag_table().lookup(TAG) {
        return t;
    }
    let t = gtk4::TextTag::new(Some(TAG));
    t.set_property("background", "#ffe58f");
    t.set_property("foreground", "#1a1a1a");
    buf.tag_table().add(&t);
    t
}

/// Removes stale hit highlighting from a buffer.
pub fn clear(buf: &gtk4::TextBuffer) {
    if let Some(t) = buf.tag_table().lookup(TAG) {
        let (s, e) = buf.bounds();
        buf.remove_tag(&t, &s, &e);
    }
}

/// Tags every literal match in `buf`, returns the count.
pub fn tag_all(buf: &gtk4::TextBuffer, needle: &str, case: bool, word: bool) -> usize {
    clear(buf);
    if needle.is_empty() {
        return 0;
    }
    let tag = find_tag(buf);
    let (mut it, end) = buf.bounds();
    let hay = buf.text(&it, &end, true).to_string();
    let (h, n) = if case {
        (hay.clone(), needle.to_string())
    } else {
        (hay.to_lowercase(), needle.to_lowercase())
    };
    let mut count = 0;
    let mut off = 0usize;
    while let Some(p) = h[off..].find(&n) {
        let a = off + p;
        let b = a + n.len();
        if word && !word_boundary(&h, a, b) {
            off = a + 1;
            continue;
        }
        let ca = a + h[..a].chars().count() - a; // bytes→chars
        let sa = h[..a].chars().count() as i32;
        let sb = sa + h[a..b].chars().count() as i32;
        let _ = ca;
        let mut i1 = buf.iter_at_offset(sa);
        let i2 = buf.iter_at_offset(sb);
        buf.apply_tag(&tag, &mut i1, &i2);
        if count == 0 {
            buf.select_range(&i1, &i2);
            it = i1;
        }
        count += 1;
        off = b;
    }
    count
}

fn word_boundary(h: &str, a: usize, b: usize) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let prev_ok = h[..a].chars().last().map_or(true, |c| !ident(c));
    let next_ok = h[b..].chars().next().map_or(true, |c| !ident(c));
    prev_ok && next_ok
}

/// Opens the Find & Replace dialog bound to `tabs`.
pub fn open(parent: &gtk4::Window, tabs: Rc<Tabs>, set_status: Rc<dyn Fn(String)>) {
    let dlg = gtk4::Window::new();
    dlg.set_title(Some(&gettextrs::gettext("Find and Replace")));
    dlg.set_transient_for(Some(parent));
    dlg.set_modal(true);
    dlg.set_default_size(420, -1);

    let find = gtk4::Entry::new();
    find.set_placeholder_text(Some(&gettextrs::gettext("Find")));
    let repl = gtk4::Entry::new();
    repl.set_placeholder_text(Some(&gettextrs::gettext("Replace with")));
    let case_cb = gtk4::CheckButton::with_label(&gettextrs::gettext("Match case"));
    let word_cb = gtk4::CheckButton::with_label(&gettextrs::gettext("Whole word"));
    let all_cb = gtk4::CheckButton::with_label(&gettextrs::gettext("Search all tabs"));
    let status = gtk4::Label::new(None);
    status.set_halign(gtk4::Align::Start);

    let grid = gtk4::Grid::new();
    grid.set_margin_top(12);
    grid.set_margin_bottom(12);
    grid.set_margin_start(12);
    grid.set_margin_end(12);
    grid.set_row_spacing(8);
    grid.set_column_spacing(8);
    grid.attach(
        &gtk4::Label::new(Some(&gettextrs::gettext("Find:"))),
        0,
        0,
        1,
        1,
    );
    grid.attach(&find, 1, 0, 3, 1);
    grid.attach(
        &gtk4::Label::new(Some(&gettextrs::gettext("Replace:"))),
        0,
        1,
        1,
        1,
    );
    grid.attach(&repl, 1, 1, 3, 1);
    grid.attach(&case_cb, 1, 2, 1, 1);
    grid.attach(&word_cb, 2, 2, 1, 1);
    grid.attach(&all_cb, 3, 2, 1, 1);

    let b_find = gtk4::Button::with_label(&gettextrs::gettext("Find next"));
    let b_repl = gtk4::Button::with_label(&gettextrs::gettext("Replace"));
    let b_all = gtk4::Button::with_label(&gettextrs::gettext("Replace all"));
    let b_close = gtk4::Button::with_label(&gettextrs::gettext("Close"));
    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    for b in [&b_find, &b_repl, &b_all, &b_close] {
        btns.append(b);
    }
    grid.attach(&btns, 0, 3, 4, 1);
    grid.attach(&status, 0, 4, 4, 1);
    dlg.set_child(Some(&grid));

    let run_find = {
        let (dlg, status, set_status, tabs) = (
            dlg.clone(),
            status.clone(),
            set_status.clone(),
            tabs.clone(),
        );
        move |find: &gtk4::Entry,
              case: &gtk4::CheckButton,
              word: &gtk4::CheckButton,
              all: &gtk4::CheckButton| {
            let needle = find.text().to_string();
            if needle.is_empty() {
                return;
            }
            let mut hits = 0usize;
            if all.is_active() {
                for (ed, page) in tabs::all(&tabs) {
                    hits += tag_all(
                        &ed.view.buffer(),
                        &needle,
                        case.is_active(),
                        word.is_active(),
                    );
                    let _ = page;
                }
            } else if let Some((ed, page)) = tabs::current(&tabs) {
                hits += tag_all(
                    &ed.view.buffer(),
                    &needle,
                    case.is_active(),
                    word.is_active(),
                );
                let _ = page;
            }
            let msg = format!("{} {}", hits, gettextrs::gettext("matches"));
            status.set_text(&msg);
            set_status(msg);
            let _ = dlg;
        }
    };

    {
        let (f, c, wd, a) = (
            find.clone(),
            case_cb.clone(),
            word_cb.clone(),
            all_cb.clone(),
        );
        b_find.connect_clicked(move |_| run_find(&f, &c, &wd, &a));
    }
    {
        let (tabs, status, repl) = (tabs.clone(), status.clone(), repl.clone());
        b_repl.connect_clicked(move |_| {
            if let Some((ed, _)) = tabs::current(&tabs) {
                let buf = ed.view.buffer();
                if let Some((mut a, mut b)) = buf.selection_bounds() {
                    buf.delete(&mut a, &mut b);
                    let mut it = buf.iter_at_mark(&buf.get_insert());
                    buf.insert(&mut it, &repl.text());
                }
                let _ = &status;
            }
        });
    }
    {
        let (tabs, status, find, repl, case_cb) = (
            tabs.clone(),
            status.clone(),
            find.clone(),
            repl.clone(),
            case_cb.clone(),
        );
        b_all.connect_clicked(move |_| {
            let needle = find.text().to_string();
            let rep = repl.text().to_string();
            let mut total = 0usize;
            for (ed, _) in tabs::all(&tabs) {
                let buf = ed.view.buffer();
                let (s, e) = buf.bounds();
                let hay = buf.text(&s, &e, true).to_string();
                let (h, n) = if case_cb.is_active() {
                    (hay.clone(), needle.clone())
                } else {
                    (hay.to_lowercase(), needle.to_lowercase())
                };
                total += h.matches(&n).count();
                if !needle.is_empty() {
                    let newt = hay.replace(&needle, &rep); // case-sensitive replace honours raw text
                    editor::set_text(&ed, &newt);
                    *ed.dirty.borrow_mut() = true;
                }
            }
            status.set_text(&format!("{} {}", total, gettextrs::gettext("replacements")));
        });
    }
    b_close.connect_clicked(move |b| {
        b.root().and_downcast::<gtk4::Window>().unwrap().close();
    });
    dlg.present();
}
