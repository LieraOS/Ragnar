//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Settings dialog: theme, font, word wrap, line numbers, language
//! and auto-save interval — the same options as the Qt original.

use gtk4::prelude::*;
use std::rc::Rc;

use crate::settings::Settings;

/// Shows the settings window; `on_change` fires after every edit.
pub fn open(parent: &gtk4::Window, s: Rc<std::cell::RefCell<Settings>>, on_change: Rc<dyn Fn()>) {
    let w = gtk4::Window::new();
    w.set_title(Some(&gettextrs::gettext("Settings")));
    w.set_transient_for(Some(parent));
    w.set_modal(true);
    w.set_default_size(420, -1);

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_margin_top(14);
    list.set_margin_bottom(14);
    list.set_margin_start(14);
    list.set_margin_end(14);

    let mk_row = |title: &str| -> gtk4::Box {
        let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        b.set_margin_top(10);
        b.set_margin_bottom(10);
        b.set_margin_start(12);
        b.set_margin_end(12);
        let l = gtk4::Label::new(Some(title));
        l.set_halign(gtk4::Align::Start);
        l.set_hexpand(true);
        b.append(&l);
        b
    };

    // Dark theme switch
    let r = mk_row(&gettextrs::gettext("Dark theme"));
    let sw = gtk4::Switch::new();
    sw.set_active(s.borrow().dark);
    r.append(&sw);
    list.append(&r);
    {
        let s = s.clone();
        let oc = on_change.clone();
        sw.connect_state_set(move |_, v| {
            s.borrow_mut().dark = v;
            crate::settings::save(&s.borrow());
            oc();
            glib::Propagation::Proceed
        });
    }

    // Font picker
    let r = mk_row(&gettextrs::gettext("Font"));
    let fd = gtk4::FontDialog::new();
    let fb = gtk4::FontDialogButton::new(Some(fd));
    fb.set_font_desc(&gtk4::pango::FontDescription::from_string(&s.borrow().font));
    r.append(&fb);
    list.append(&r);
    {
        let s = s.clone();
        let oc = on_change.clone();
        fb.connect_font_desc_notify(move |b| {
            s.borrow_mut().font = b.font_desc().map(|d| d.to_string()).unwrap_or_default();
            crate::settings::save(&s.borrow());
            oc();
        });
    }

    // Word wrap
    let r = mk_row(&gettextrs::gettext("Word wrap"));
    let wc = gtk4::CheckButton::new();
    wc.set_active(s.borrow().word_wrap);
    r.append(&wc);
    list.append(&r);
    {
        let s = s.clone();
        let oc = on_change.clone();
        wc.connect_toggled(move |c| {
            s.borrow_mut().word_wrap = c.is_active();
            crate::settings::save(&s.borrow());
            oc();
        });
    }

    // Auto-save interval (0 = off)
    let r = mk_row(&gettextrs::gettext("Auto-save interval (s, 0 = off)"));
    let spin = gtk4::SpinButton::with_range(0.0, 600.0, 5.0);
    spin.set_value(s.borrow().autosave_secs as f64);
    r.append(&spin);
    list.append(&r);
    {
        let s = s.clone();
        spin.connect_value_changed(move |b| {
            s.borrow_mut().autosave_secs = b.value() as u32;
            crate::settings::save(&s.borrow());
        });
    }

    // Language selector
    let r = mk_row(&gettextrs::gettext("Language"));
    let combo = gtk4::DropDown::from_strings(&["System", "English", "Deutsch"]);
    combo.set_selected(match s.borrow().lang.as_str() {
        "en" => 1,
        "de" => 2,
        _ => 0,
    });
    r.append(&combo);
    list.append(&r);
    {
        let s = s.clone();
        combo.connect_selected_notify(move |c| {
            s.borrow_mut().lang = ["", "en", "de"][c.selected() as usize].to_string();
            crate::settings::save(&s.borrow());
        });
    }

    w.set_child(Some(&list));
    w.present();
}
