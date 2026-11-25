// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — preferences dialog
//
// language, auto-refresh toggle,
// last-used devices and profile storage() — persisted through
// core::preferences (KeyFile INI in ~/.config/ragnar-sap/).

use gtk4::prelude::*;

use crate::core::preferences::Preferences;
use crate::i18n::tr;

/// Open the preferences dialog as a modal child of `parent`.
pub fn present_prefs(parent: &impl IsA<gtk4::Widget>) {
 let window = gtk4::Window::builder()
.title(tr("Einstellungen"))
.transient_for(&parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok()).unwrap_or_default())
.modal(true)
.default_width(440)
.build();

 let header = gtk4::HeaderBar::new();
 window.set_titlebar(Some(&header));

 let root = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
 root.set_margin_top(14);
 root.set_margin_bottom(14);
 root.set_margin_start(14);
 root.set_margin_end(14);

 let prefs = Preferences::load();
 let current = prefs.get();
 let prefs = std::rc::Rc::new(std::cell::RefCell::new(prefs));

 // Language — msgids are German; English maps through gettext.
 root.append(&gtk4::Label::builder().label(tr("Sprache")).xalign(0.0).build());
 let lang_dd = gtk4::DropDown::from_strings(&[tr("Systemsprache").as_str(), "Deutsch", "English"]);
 lang_dd.set_selected(match current.language.as_str() {
 "de" => 1,
 "en" => 2,
 _ => 0,
 });
 root.append(&lang_dd);

 // Auto-refresh switch (dashboard device list).
 let refresh_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 refresh_row.append(&gtk4::Label::builder()
.label(tr("Geräteliste automatisch aktualisieren"))
.xalign(0.0)
.hexpand(true)
.build());
 let refresh_sw = gtk4::Switch::new();
 refresh_sw.set_active(current.auto_refresh_devices);
 refresh_row.append(&refresh_sw);
 root.append(&refresh_row);

 // Font size — needed a whole theme system; GTK renders via
 // the system theme, so only the text scaling preference remains.
 let font_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 font_row.append(&gtk4::Label::new(Some(&tr("Schriftgröße"))));
 let font_spin = gtk4::SpinButton::with_range(8.0, 24.0, 1.0);
 font_spin.set_value(current.font_size as f64);
 font_spin.set_hexpand(true);
 font_row.append(&font_spin);
 root.append(&font_row);

 let save = gtk4::Button::with_label(&tr("Speichern"));
 save.add_css_class("suggested-action");
 root.append(&save);

 let status = gtk4::Label::new(Some(""));
 root.append(&status);
 window.set_child(Some(&root));

 save.connect_clicked({
 let prefs = prefs.clone();
 let status = status.clone();
 let window = window.clone();
 move |_| {
 let mut p = prefs.borrow_mut();
 let mut u = p.get();
 u.language = match lang_dd.selected() {
 1 => "de".into(),
 2 => "en".into(),
 _ => "system".into(),
 };
 u.auto_refresh_devices = refresh_sw.is_active();
 u.font_size = font_spin.value() as i32;
 let new_lang = u.language.clone();
 p.set(u);
 if !p.save_to_file(&crate::core::preferences::config_path().to_string_lossy()) {
 status.set_label(&tr("Speichern fehlgeschlagen."));
 return;
 }
 status.set_label(&tr("Gespeichert."));
 // Instant language switch — same feel as the rest of
 // Ragnar: apply, then rebuild the visible windows.
 crate::i18n::set_language(&new_lang);
 let w = window.clone();
 gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
 let parent = w.transient_for().clone();
 w.close();
 if let Some(dash) = parent.and_then(|w| w.downcast::<gtk4::ApplicationWindow>().ok()) {
 if let Some(app) = dash.application() {
 super::present_dashboard(&app);
 dash.close();
 }
 }
 });
 }
 });

 window.present();
}
