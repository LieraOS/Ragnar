// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-run-dialog — the overlay launcher window
//
// A wlr-layer-shell Overlay surface anchored to the top edge and
// centred horizontally, with an exclusive keyboard grab while open.
// It closes on Escape, after a successful launch, and when focus is
// lost — the usual launcher contract. Works on any compositor that
// speaks wlr-layer-shell (labwc today, Ragnar's own compositor
// later); it deliberately uses no compositor-specific extensions.

use crate::i18n::tr;
use crate::{apps, css, launch, model};
use gtk4::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Cap the unfiltered list so opening stays instant.
const INITIAL_SHOWN: usize = 50;
/// Cap filtered results — typing more characters refines anyway.
const RESULTS_SHOWN: usize = 30;
/// Distance from the top edge of the monitor, in pixels.
const TOP_MARGIN: i32 = 160;

/// Shared dialog state: the enumerated apps, the rows currently
/// backing the list widget (index-aligned), and the widgets.
struct State {
    apps: Vec<model::AppEntry>,
    rows: Vec<model::Row>,
    list: gtk4::ListBox,
    entry: gtk4::SearchEntry,
}

/// Build the overlay, populate it and present it on the focused
/// monitor (the compositor picks the output when none is pinned).
pub fn present(app: &adw::Application) {
    css::load();

    let win = adw::ApplicationWindow::builder().application(app).build();
    win.add_css_class("run-dialog");
    // LayerShell is implemented on gtk4::Window — upcast for the
    // surface configuration, then keep working with the adw window.
    let surface: &gtk4::Window = win.upcast_ref();
    surface.init_layer_shell();
    surface.set_layer(Layer::Overlay);
    surface.set_namespace(Some("ragnar-run-dialog"));
    surface.set_anchor(Edge::Top, true);
    surface.set_margin(Edge::Top, TOP_MARGIN);
    // Exclusive: every keypress is ours while the dialog is up.
    surface.set_keyboard_mode(KeyboardMode::Exclusive);

    let entry = gtk4::SearchEntry::new();
    entry.add_css_class("run-entry");
    entry.set_placeholder_text(Some(&tr("Anwendung oder Befehl eingeben …")));

    let list = gtk4::ListBox::new();
    list.add_css_class("run-list");
    list.set_selection_mode(gtk4::SelectionMode::Single);

    // Our own scrolled window — the 320 px popover quirk from GTK's
    // PopoverMenu template does not apply here; still cap growth so
    // a huge result set cannot push the window off-screen.
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(420)
        .child(&list)
        .build();
    scrolled.add_css_class("run-results");

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.add_css_class("run-content");
    content.append(&entry);
    content.append(&scrolled);
    win.set_content(Some(&content));

    let st = Rc::new(RefCell::new(State {
        apps: apps::installed(),
        rows: Vec::new(),
        list: list.clone(),
        entry: entry.clone(),
    }));

    // Typing filters; Enter in the entry activates the selection.
    {
        let st = st.clone();
        entry.connect_search_changed(move |e| rebuild(&st, &e.text()));
    }
    {
        let st = st.clone();
        let win = win.clone();
        entry.connect_activate(move |_| activate_selected(&st, &win));
    }
    // SearchEntry swallows Escape for its own stop-search signal —
    // the window-level key controller never sees it while the entry
    // has focus, so closing has to hook in here as well.
    {
        let win = win.clone();
        entry.connect_stop_search(move |_| win.close());
    }

    // Arrow keys move the highlighted row without leaving the entry.
    let nav = gtk4::EventControllerKey::new();
    {
        let st = st.clone();
        nav.connect_key_pressed(move |_, key, _, _| {
            let mut s = st.borrow_mut();
            match key {
                gdk::Key::Down => move_selection(&mut s, 1),
                gdk::Key::Up => move_selection(&mut s, -1),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
    }
    entry.add_controller(nav);

    // Click (or Enter on a focused row) launches.
    {
        let st = st.clone();
        let win = win.clone();
        list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            let rk = st.borrow().rows.get(idx).map(|r| match r {
                model::Row::App(i) => model::Row::App(*i),
                model::Row::Command(c) => model::Row::Command(c.clone()),
            });
            if let Some(rk) = rk {
                finish(&st, &win, rk);
            }
        });
    }

    // Escape closes; losing focus closes (clicked elsewhere).
    let esc = gtk4::EventControllerKey::new();
    {
        let win = win.clone();
        esc.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                win.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    win.add_controller(esc);

    // Closing on outside interaction: a layer-shell surface never
    // sees clicks that land on other surfaces, and with an exclusive
    // keyboard grab the compositor sends no focus-out — so the
    // reliable signal is the pointer leaving our surface. `leave`
    // only fires after an `enter`, so keyboard-only use (pointer
    // never inside) never triggers it.
    let pointer = gtk4::EventControllerMotion::new();
    {
        let win = win.clone();
        pointer.connect_leave(move |_| win.close());
    }
    win.add_controller(pointer);
    win.connect_is_active_notify(|w| {
        if !w.is_active() {
            w.close();
        }
    });

    rebuild(&st, "");
    win.present();
    entry.grab_focus();
}

/// Rebuild the result list for `query`: scored app matches plus a
/// trailing "run command" row whenever the text is non-empty.
fn rebuild(st: &Rc<RefCell<State>>, query: &str) {
    let mut s = st.borrow_mut();
    while let Some(child) = s.list.first_child() {
        s.list.remove(&child);
    }
    s.rows.clear();

    let q = query.trim();
    let idxs: Vec<usize> = if q.is_empty() {
        (0..s.apps.len().min(INITIAL_SHOWN)).collect()
    } else {
        apps::search(&s.apps, q)
            .into_iter()
            .take(RESULTS_SHOWN)
            .collect()
    };
    for i in idxs {
        s.list.append(&app_row(&s.apps[i]));
        s.rows.push(model::Row::App(i));
    }
    if !q.is_empty() {
        s.list.append(&command_row(q));
        s.rows.push(model::Row::Command(q.to_string()));
    }

    // Keep the first row highlighted so Enter does something sane.
    if let Some(first) = s.list.row_at_index(0) {
        s.list.select_row(Some(&first));
    }
}

/// One app result: icon + name, dimmed description as second line.
fn app_row(e: &model::AppEntry) -> gtk4::ListBoxRow {
    let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    h.add_css_class("run-row");

    let icon = e
        .info
        .icon()
        .map(|g| gtk4::Image::from_gicon(&g))
        .unwrap_or_else(|| gtk4::Image::from_icon_name("application-x-executable"));
    icon.add_css_class("run-row-icon");
    h.append(&icon);

    let text = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let name = gtk4::Label::new(Some(&e.name));
    name.add_css_class("run-row-name");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    text.append(&name);
    if !e.description.is_empty() {
        let desc = gtk4::Label::new(Some(&e.description));
        desc.add_css_class("run-row-desc");
        desc.set_xalign(0.0);
        desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        text.append(&desc);
    }
    h.append(&text);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&h));
    row
}

/// The fallback row that runs the typed text as a command line.
fn command_row(cmd: &str) -> gtk4::ListBoxRow {
    let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    h.add_css_class("run-row");
    h.add_css_class("run-command-row");

    let icon = gtk4::Image::from_icon_name("system-run");
    icon.add_css_class("run-row-icon");
    h.append(&icon);

    let text = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let name = gtk4::Label::new(Some(&tr("Befehl ausführen")));
    name.add_css_class("run-row-name");
    name.set_xalign(0.0);
    text.append(&name);
    let desc = gtk4::Label::new(Some(cmd));
    desc.add_css_class("run-row-desc");
    desc.set_xalign(0.0);
    desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    text.append(&desc);
    h.append(&text);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&h));
    row
}

/// Shift the highlighted row by `delta`, clamped to the list. The
/// row focus grab scrolls it into view; focus goes straight back to
/// the entry so typing keeps working.
fn move_selection(s: &mut State, delta: i32) {
    let current = s.list.selected_row().map(|r| r.index()).unwrap_or(-1);
    let next = (current + delta).clamp(0, s.rows.len() as i32 - 1);
    if let Some(row) = s.list.row_at_index(next) {
        s.list.select_row(Some(&row));
        row.grab_focus();
        s.entry.grab_focus();
    }
}

/// Activate whatever row is currently highlighted.
fn activate_selected(st: &Rc<RefCell<State>>, win: &adw::ApplicationWindow) {
    let rk = {
        let s = st.borrow();
        s.list
            .selected_row()
            .and_then(|r| s.rows.get(r.index() as usize))
            .map(|r| match r {
                model::Row::App(i) => model::Row::App(*i),
                model::Row::Command(c) => model::Row::Command(c.clone()),
            })
    };
    if let Some(rk) = rk {
        finish(st, win, rk);
    }
}

/// Launch the chosen row; close only when it actually started.
fn finish(st: &Rc<RefCell<State>>, win: &adw::ApplicationWindow, rk: model::Row) {
    let ok = {
        let s = st.borrow();
        gdk::Display::default()
            .map(|d| launch::run(&rk, &s.apps, &d))
            .unwrap_or(false)
    };
    if ok {
        win.close();
    }
}
