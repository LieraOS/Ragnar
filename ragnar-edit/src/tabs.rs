//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Tab management on adw::TabView/TabBar: page titles with dirty
//! indicators, close handling and the currently-active Editor.

use libadwaita as adw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::editor::{self, Editor};

pub struct Tabs {
    pub view: adw::TabView,
    pub bar: adw::TabBar,
    /// Maps each TabPage to its Editor by page pointer identity.
    editors: Rc<RefCell<HashMap<usize, Rc<Editor>>>>,
    untitled: Rc<RefCell<u32>>,
}

pub fn new() -> Tabs {
    let view = adw::TabView::new();
    let bar = adw::TabBar::new();
    bar.set_view(Some(&view));
    bar.set_autohide(true);
    let editors = Rc::new(RefCell::new(HashMap::new()));
    {
        let eds = editors.clone();
        view.connect_close_page(move |_, page| {
            eds.borrow_mut().remove(&key(page));
            glib::Propagation::Proceed
        });
    }
    Tabs {
        view,
        bar,
        editors,
        untitled: Rc::new(RefCell::new(1)),
    }
}

/// Stable identity: the underlying GObject pointer — TabPage
/// wrappers are re-created per call, the object is not.
fn key(page: &adw::TabPage) -> usize {
    use glib::object::ObjectType;
    page.as_ptr() as usize
}

/// Adds a fresh, empty tab and returns its editor + page.
pub fn add(t: &Tabs, font: &str, wrap: bool, lnum: bool) -> (Rc<Editor>, adw::TabPage) {
    let ed = editor::new(font, wrap, lnum);
    let page = t.view.append(&ed.scrolled);
    let n = *t.untitled.borrow();
    page.set_title(&format!(
        "{}{}",
        gettextrs::gettext("Untitled"),
        if n > 1 {
            format!(" {}", n)
        } else {
            String::new()
        }
    ));
    *t.untitled.borrow_mut() = n + 1;
    page.set_icon(Some(&gtk4::gio::ThemedIcon::new("text-x-generic-symbolic")));
    let _ = lnum;
    t.editors.borrow_mut().insert(key(&page), ed.clone());
    (ed, page)
}

/// Current page's editor, if any.
pub fn current(t: &Tabs) -> Option<(Rc<Editor>, adw::TabPage)> {
    let page = t.view.selected_page()?;
    let ed = t.editors.borrow().get(&key(&page)).cloned()?;
    Some((ed, page))
}

pub fn editor_for(t: &Tabs, page: &adw::TabPage) -> Option<Rc<Editor>> {
    t.editors.borrow().get(&key(page)).cloned()
}

/// Sets the page title from a file path, dropping the Untitled label.
pub fn retitle(page: &adw::TabPage, path: &std::path::Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    page.set_title(name);
    page.set_tooltip(&path.display().to_string());
}

/// Refreshes the dirty indicator on a page (adw dot).
pub fn set_dirty_indicator(page: &adw::TabPage, dirty: bool) {
    page.set_needs_attention(dirty);
}

/// Closes the page and forgets its editor.
pub fn close(t: &Tabs, page: &adw::TabPage) {
    t.editors.borrow_mut().remove(&key(page));
    t.view.close_page(page);
}

/// Iterates all open (editor, page) pairs — used by multi-tab search
/// and autosave.
pub fn all(t: &Tabs) -> Vec<(Rc<Editor>, adw::TabPage)> {
    let mut v = Vec::new();
    for i in 0..t.view.n_pages() {
        let p = t.view.nth_page(i);
        if let Some(e) = editor_for(t, &p) {
            v.push((e, p));
        }
    }
    v
}
