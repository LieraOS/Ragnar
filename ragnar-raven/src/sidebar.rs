//! The Raven window: a compact layer-shell surface anchored to the
//! top-right corner of the monitor — NOT a full-height sidebar. It
//! opens for one concrete day (picked in the taskbar calendar) and
//! shows that day's appointments plus the inline editor for new
//! entries, and below a compact notification area. The month calendar
//! deliberately stays in the taskbar clock panel.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

use crate::dialog;
use crate::i18n::tr;
use crate::store::Store;

/// Widgets that get refreshed when the day or the store changes.
struct Ui {
    day_head: gtk4::Label,
    day_list: gtk4::Box,
    window: gtk4::Window,
    day: std::cell::RefCell<glib::DateTime>,
}

/// Build the compact Raven window for `day` and wire all callbacks.
fn build_ui(store: &Store, day: &glib::DateTime) -> std::rc::Rc<Ui> {
    let window = gtk4::Window::new();
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    // Compact top-right strip: anchored right and top only, so the
    // height hugs the content instead of spanning the monitor.
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Top, true);
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
    window.set_default_width(360);
    window.set_margin(Edge::Top, 8);
    window.set_margin(Edge::Right, 8);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    body.set_margin_top(10);
    body.set_margin_bottom(10);
    body.set_margin_start(12);
    body.set_margin_end(12);
    body.add_css_class("raven");

    // Header row: day headline plus a close button — the window is a
    // transient popup, so it must be dismissable without the editor.
    let head_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let day_head = gtk4::Label::new(None);
    day_head.set_halign(gtk4::Align::Start);
    day_head.set_hexpand(true);
    head_row.append(&day_head);
    let close = gtk4::Button::from_icon_name("window-close-symbolic");
    close.set_has_frame(false);
    close.connect_clicked({
        let w = window.clone();
        move |_| w.close()
    });
    head_row.append(&close);
    body.append(&head_row);

    // This day's existing appointments (compact rows).
    let day_list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    body.append(&day_list);

    // New appointment entry point → editor dialog.
    let add = gtk4::Button::with_label(&tr("Termin eintragen"));
    add.set_halign(gtk4::Align::Start);
    add.add_css_class("flat");
    body.append(&add);

    body.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // Compact notification area: live history collected by the
    // org.freedesktop.Notifications server part of this process.
    let hist_head = gtk4::Label::new(None);
    hist_head.set_markup(&format!("<b>{}</b>", tr("Benachrichtigungen")));
    hist_head.set_halign(gtk4::Align::Start);
    body.append(&hist_head);
    let items = crate::notify::history();
    if items.is_empty() {
        let hist = gtk4::Label::new(Some(&tr("Keine Benachrichtigungen")));
        hist.set_halign(gtk4::Align::Start);
        body.append(&hist);
    } else {
        for it in items.iter().take(5) {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            if !it.icon.is_empty() {
                let img = gtk4::Image::from_icon_name(&it.icon);
                img.set_pixel_size(16);
                row.append(&img);
            }
            let l = gtk4::Label::new(None);
            l.set_markup(&format!(
                "<b>{}</b>  {}",
                glib::markup_escape_text(&it.summary),
                glib::markup_escape_text(&it.when)
            ));
            l.set_halign(gtk4::Align::Start);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row.append(&l);
            body.append(&row);
        }
    }

    window.set_child(Some(&body));

    let ui = std::rc::Rc::new(Ui {
        day_head,
        day_list,
        window,
        day: std::cell::RefCell::new(day.clone()),
    });

    add.connect_clicked({
        let (ui, st) = (ui.clone(), store.clone());
        move |_| {
            let day = ui.day.borrow().clone();
            open_editor_paired(&ui, &st, &day, None);
        }
    });

    // Escape also dismisses the transient window.
    let keys = gtk4::EventControllerKey::new();
    keys.connect_key_pressed({
        let w = ui.window.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                w.close();
            }
            glib::Propagation::Proceed
        }
    });
    ui.window.add_controller(keys);

    refresh(&ui, store);
    ui
}

/// Re-render headline and appointment rows for the stored day.
fn refresh(ui: &std::rc::Rc<Ui>, store: &Store) {
    let day = ui.day.borrow().clone();
    ui.day_head
        .set_markup(&format!("<b>{}</b>", day.format("%A, %d. %B %Y").unwrap()));
    while let Some(c) = ui.day_list.first_child() {
        ui.day_list.remove(&c);
    }
    let items = store.on_day(&day);
    if items.is_empty() {
        let l = gtk4::Label::new(Some(&tr("Keine Termine")));
        l.set_halign(gtk4::Align::Start);
        ui.day_list.append(&l);
        return;
    }
    for a in items {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let when = gtk4::Label::new(None);
        when.set_markup(&format!(
            "<b>{}</b>  {}",
            a.start.format("%H:%M").unwrap(),
            glib::markup_escape_text(&a.title)
        ));
        when.set_halign(gtk4::Align::Start);
        let texts = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        texts.set_hexpand(true);
        texts.append(&when);
        if !a.desc.is_empty() {
            let d = gtk4::Label::new(Some(&a.desc));
            d.set_halign(gtk4::Align::Start);
            d.add_css_class("dim-label");
            d.add_css_class("caption");
            d.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            texts.append(&d);
        }
        texts.set_valign(gtk4::Align::Center);
        row.append(&texts);
        // Edit reuses the same editor, pre-filled; saving keeps the
        // UID so fired/dedup tracking and external .ics consumers
        // still recognise the appointment.
        let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
        edit.set_has_frame(false);
        edit.connect_clicked({
            let (ui, st, appt) = (ui.clone(), store.clone(), a.clone());
            move |_| open_editor_paired(&ui, &st, &appt.start, Some(&appt))
        });
        row.append(&edit);
        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        del.set_has_frame(false);
        let uid = a.uid.clone();
        del.connect_clicked({
            let st = store.clone();
            let ui = ui.clone();
            move |_| {
                st.remove(&uid);
                refresh(&ui, &st);
            }
        });
        row.append(&del);
        ui.day_list.append(&row);
    }
}

/// Open the appointment editor paired to the day window — when the
/// editor closes (saved or cancelled), the day view closes with it.
/// `existing` selects edit mode (pre-filled, UID kept); None adds.
fn open_editor_paired(
    ui: &std::rc::Rc<Ui>,
    store: &Store,
    day: &glib::DateTime,
    existing: Option<&crate::store::Appointment>,
) {
    let ed = dialog::open_editor(ui.window.upcast_ref(), day, store, existing);
    ed.connect_close_request({
        let w = ui.window.clone();
        move |_| {
            w.close();
            glib::Propagation::Proceed
        }
    });
}

thread_local! {
    /// The single live day window — a second --day activation must
    /// retarget it instead of stacking another window at the edge.
    static ACTIVE: std::cell::RefCell<Option<std::rc::Rc<Ui>>> =
        const { std::cell::RefCell::new(None) };
}

/// Open the compact Raven window for the given day. If one is
/// already open it is reused: day switched, content refreshed,
/// window raised — never a second stacked window.
pub fn open(app: &gtk4::Application, store: &Store, day: &glib::DateTime) {
    if let Some(ui) = ACTIVE.with(|a| a.borrow().clone()) {
        *ui.day.borrow_mut() = day.clone();
        refresh(&ui, store);
        ui.window.present();
        return;
    }
    let ui = build_ui(store, day);
    app.add_window(&ui.window);
    // Clear the slot no matter which path closes the window —
    // close button, Escape, or the editor's transient pairing.
    ui.window.connect_close_request(|_| {
        ACTIVE.with(|a| *a.borrow_mut() = None);
        glib::Propagation::Proceed
    });
    ACTIVE.with(|a| *a.borrow_mut() = Some(ui.clone()));
    ui.window.present();
}
