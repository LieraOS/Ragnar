// Copyright (C) Jörg Schröder
//! Dialogs used by the context menus: Open With and Rename. They
//! are ordinary toplevels with a header bar (close button, Esc) —
//! deliberately not modal/transient to the layer-shell desktop
//! surface, which cannot take focus away from them.

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;

use crate::i18n::tr;

thread_local! {
    /// Dialogs currently open — closed together when the user clicks
    /// the bare desktop.
    static OPEN: std::cell::RefCell<Vec<adw::Window>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Close every open dialog (a click on the desktop dismisses them).
pub fn close_all() {
    let all: Vec<adw::Window> = OPEN.with(|o| o.borrow().clone());
    for w in all {
        w.close();
    }
}

/// A titled window with header bar around `child`; Esc closes it.
fn shell(title: &str, w: i32, h: i32, child: &impl IsA<gtk4::Widget>) -> adw::Window {
    let win = adw::Window::builder()
        .title(title)
        .default_width(w)
        .default_height(h)
        .build();
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&adw::HeaderBar::new());
    tv.set_content(Some(child));
    win.set_content(Some(&tv));
    let key = gtk4::EventControllerKey::new();
    let w2 = win.clone();
    key.connect_key_pressed(move |_, k, _, _| {
        if k == gtk4::gdk::Key::Escape {
            w2.close();
        }
        gtk4::glib::Propagation::Proceed
    });
    win.add_controller(key);
    OPEN.with(|o| o.borrow_mut().push(win.clone()));
    win.connect_close_request(|w| {
        OPEN.with(|o| o.borrow_mut().retain(|x| x != w));
        gtk4::glib::Propagation::Proceed
    });
    win
}

/// Confirmation dialog: `on_ok` runs only when the destructive
/// button is pressed.
pub fn confirm(title: &str, body: &str, action: &str, on_ok: Rc<dyn Fn()>) {
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    for m in ["top", "bottom", "start", "end"] {
        content.set_property(&format!("margin-{m}"), 18);
    }
    let msg = gtk4::Label::new(Some(body));
    msg.set_wrap(true);
    msg.set_xalign(0.0);
    content.append(&msg);
    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&tr("Cancel"));
    let ok = gtk4::Button::with_label(action);
    ok.add_css_class("destructive-action");
    buttons.append(&cancel);
    buttons.append(&ok);
    content.append(&buttons);
    let win = shell(title, 380, -1, &content);
    let w = win.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = win.clone();
    ok.connect_clicked(move |_| {
        on_ok();
        w.close();
    });
    win.present();
}

/// Entry dialog asking for a name; runs `on_done(name)` on confirm.
pub fn name_prompt(title: &str, initial: &str, on_done: Rc<dyn Fn(String)>) {
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    for m in ["top", "bottom", "start", "end"] {
        content.set_property(&format!("margin-{m}"), 18);
    }
    let entry = gtk4::Entry::builder().text(initial).build();
    let ok = gtk4::Button::with_label(&tr("OK"));
    ok.add_css_class("suggested-action");
    ok.set_halign(gtk4::Align::End);
    content.append(&entry);
    content.append(&ok);
    let win = shell(title, 360, -1, &content);
    let confirm = {
        let (e, w) = (entry.clone(), win.clone());
        move || {
            on_done(e.text().to_string());
            w.close();
        }
    };
    let c2 = confirm.clone();
    entry.connect_activate(move |_| c2());
    ok.connect_clicked(move |_| confirm());
    win.present();
    entry.grab_focus();
}

/// One list row: app icon + name.
fn app_row(app: &gio::AppInfo) -> gtk4::ListBoxRow {
    let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    for m in ["top", "bottom", "start", "end"] {
        h.set_property(&format!("margin-{m}"), 8);
    }
    let icon = app
        .icon()
        .unwrap_or_else(|| gio::ThemedIcon::new("application-x-executable").upcast());
    let img = gtk4::Image::from_gicon(&icon);
    img.set_pixel_size(24);
    h.append(&img);
    h.append(&gtk4::Label::new(Some(&app.name())));
    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&h));
    row
}

/// A list of apps; activating a row launches that app on `file`
/// and closes `win` (resolved lazily, the window wraps the list).
fn app_list(
    apps: Vec<gio::AppInfo>,
    file: &gio::File,
    win: Rc<std::cell::RefCell<Option<adw::Window>>>,
) -> gtk4::ListBox {
    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    for a in &apps {
        list.append(&app_row(a));
    }
    let f = file.clone();
    list.connect_row_activated(move |_, row| {
        if let Some(app) = apps.get(row.index() as usize) {
            if let Err(e) = app.launch(std::slice::from_ref(&f), None::<&gio::AppLaunchContext>) {
                eprintln!("[ragnar-view] launch {} failed: {e}", app.name());
            }
        }
        if let Some(w) = win.borrow().as_ref() {
            w.close();
        }
    });
    list
}

/// "Open With": recommended handlers first, every other installed
/// application in a collapsed section below.
pub fn open_with(file: &gio::File) {
    let Ok(info) = file.query_info(
        "standard::content-type",
        gio::FileQueryInfoFlags::NONE,
        None::<&gio::Cancellable>,
    ) else {
        return;
    };
    let ct = info.content_type().unwrap_or_default();
    let recommended = gio::AppInfo::recommended_for_type(&ct);
    let mut others: Vec<gio::AppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter(|a| a.should_show() && !recommended.iter().any(|r| r.id() == a.id()))
        .collect();
    others.sort_by_key(|a| a.name().to_lowercase());

    let win_ref = Rc::new(std::cell::RefCell::new(None));
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 12);
    }
    body.append(&app_list(recommended, file, win_ref.clone()));
    let more = gtk4::Expander::new(Some(&tr("Other applications")));
    more.set_child(Some(&app_list(others, file, win_ref.clone())));
    body.append(&more);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&body));
    scroll.set_vexpand(true);

    let win = shell(&tr("Open With"), 380, 480, &scroll);
    win_ref.replace(Some(win.clone()));
    win.present();
}
