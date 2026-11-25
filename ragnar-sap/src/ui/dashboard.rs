// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — main dashboard window
//
// device overview (scanners + printers with
// online/offline state), action buttons for the workflows, and a
// status line. Discovery runs on a worker thread; the list refreshes
// automatically when the preferences enable auto-refresh.

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::core::device_info::{DeviceInfo, DeviceType};
use crate::core::{discovery, preferences::Preferences, registry::DeviceRegistry};
use crate::i18n::tr;
use super::{pdf_window, prefs_dialog, print_only, scan_print, worker};

/// Auto-refresh cadence (same 5 s dashboard used).
const REFRESH_SECS: u32 = 5;

/// True while a discovery pass is running — clicks and the
/// auto-refresh tick are ignored instead of queueing up.
static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Clears the in-flight flag when the worker finishes — even on
/// panic, so a failed pass cannot wedge the refresh button.
struct RefreshGuard;
impl Drop for RefreshGuard {
 fn drop(&mut self) {
 REFRESHING.store(false, std::sync::atomic::Ordering::SeqCst);
 }
}

/// Build and present the main window for `app`.
pub fn present_dashboard(app: &gtk4::Application) {
 let window = gtk4::ApplicationWindow::builder()
.application(app)
.title(tr("Ragnar SAP – Geräteübersicht"))
.default_width(760)
.default_height(560)
.build();

 let header = gtk4::HeaderBar::new();
 let refresh = gtk4::Button::from_icon_name("view-refresh-symbolic");
 refresh.set_tooltip_text(Some(&tr("Aktualisieren")));
 header.pack_start(&refresh);

 let menu = gio::Menu::new();
 menu.append(Some(&tr("Einstellungen")), Some("app.prefs"));
 menu.append(Some(&tr("Über Ragnar SAP")), Some("app.about"));
 let menu_btn = gtk4::MenuButton::builder()
.icon_name("open-menu-symbolic")
.menu_model(&menu)
.build();
 header.pack_end(&menu_btn);
 window.set_titlebar(Some(&header));

 let root = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
 root.set_margin_top(12);
 root.set_margin_bottom(12);
 root.set_margin_start(12);
 root.set_margin_end(12);

 root.append(&gtk4::Label::builder()
.label(tr("Dokumente erfassen, schwärzen und sofort drucken."))
.build());

 // Device list: one row per scanner/printer, updated on refresh.
 let list = gtk4::ListBox::new();
 list.set_selection_mode(gtk4::SelectionMode::None);
 list.add_css_class("boxed-list");
 let scroll = gtk4::ScrolledWindow::builder()
.child(&list)
.vexpand(true)
.build();
 root.append(&scroll);

 // Workflow buttons — each opens its dedicated window.
 let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 buttons.set_homogeneous(true);
 let btn_scan = gtk4::Button::with_label(&tr("Scannen && Drucken"));
 let btn_print = gtk4::Button::with_label(&tr("Nur Drucken"));
 let btn_pdf = gtk4::Button::with_label(&tr("PDF-Verwaltung"));
 let btn_prefs = gtk4::Button::with_label(&tr("Einstellungen"));
 for b in [&btn_scan, &btn_print, &btn_pdf, &btn_prefs] {
 buttons.append(b);
 }
 root.append(&buttons);

 let status = gtk4::Label::builder()
.label(tr("System bereit."))
.xalign(0.0)
.build();
 root.append(&status);
 window.set_child(Some(&root));

 // Shared device registry — repopulated on every refresh and
 // handed to the workflow windows for their device combos.
 let registry = std::rc::Rc::new(DeviceRegistry::new());

 // ---- wiring --------------------------------------------------
 refresh.connect_clicked({
 let list = list.clone();
 let status = status.clone();
 let reg = registry.clone();
 move |_| start_refresh(&list, &status, &reg)
 });

 btn_scan.connect_clicked({
 let w = window.clone();
 let reg = registry.clone();
 move |_| scan_print::present_scan_print(&w, reg.clone())
 });
 btn_print.connect_clicked({
 let w = window.clone();
 let reg = registry.clone();
 move |_| print_only::present_print_only(&w, reg.clone())
 });
 btn_pdf.connect_clicked({
 let w = window.clone();
 move |_| pdf_window::present_pdf_window(&w)
 });
 btn_prefs.connect_clicked({
 let w = window.clone();
 move |_| prefs_dialog::present_prefs(&w)
 });

 // Auto-refresh on the same cadence dashboard used, when
 // the user preference enables it.
 let prefs = Preferences::load();
 if prefs.get().auto_refresh_devices {
 let list_c = list.clone();
 let status_c = status.clone();
 let reg_c = registry.clone();
 glib::timeout_add_seconds_local(REFRESH_SECS, move || {
 start_refresh(&list_c, &status_c, &reg_c);
 glib::ControlFlow::Continue
 });
 }

 start_refresh(&list, &status, &registry);
 window.present();
}

/// Discover devices on a worker thread and repaint the list.
fn start_refresh(list: &gtk4::ListBox, status: &gtk4::Label, registry: &std::rc::Rc<DeviceRegistry>) {
 if REFRESHING.swap(true, std::sync::atomic::Ordering::SeqCst) {
 return;
 }
 // "Searching" placeholders only while the list is still empty —
 // later refreshes keep the current rows until the worker returns.
 if registry.all().is_empty() {
 render(list, None);
 }
 status.set_label(&tr("Suche nach Geräten…"));
 let list_c = list.clone();
 let status_c = status.clone();
 let reg_c = registry.clone();
 worker::spawn(|| {
 let _guard = RefreshGuard;
 let reg = DeviceRegistry::new();
 discovery::discover_all(&reg);
 reg.all()
 },
 move |devices: Vec<DeviceInfo>| {
 // Repopulate the shared registry — the worker discovers into a
 // private instance because Rc cannot cross threads; the windows
 // get their combos from this one.
 reg_c.clear();
 for d in &devices {
 reg_c.register(d.clone());
 }
 render(&list_c, Some(&devices));
 status_c.set_label(&tr("Geräte aktualisiert."));
 },
 );
}

/// Rebuild the device rows — scanner group first, then printers.
/// Both section headers always show so the page never looks dead:
/// `None` means discovery is still running (a spinner row under
/// each header), `Some` paints the found devices or a dim
/// "none found" hint per section.
fn render(list: &gtk4::ListBox, devices: Option<&[DeviceInfo]>) {
 while let Some(row) = list.first_child() {
 list.remove(&row);
 }
 for kind in [DeviceType::Scanner, DeviceType::Printer] {
 let title = match kind {
 DeviceType::Scanner => tr("Scanner"),
 DeviceType::Printer => tr("Drucker"),
 };
 let header_row = gtk4::Label::builder()
.label(format!("<b>{title}</b>"))
.use_markup(true)
.xalign(0.0)
.margin_start(8)
.margin_top(8)
.build();
 list.append(&header_row);
 match devices {
 None => list.append(&pending_row()),
 Some(devs) => {
 let mut any = false;
 for dev in devs.iter().filter(|d| d.device_type == kind) {
 any = true;
 let state = if dev.online { tr("online") } else { tr("offline") };
 let row = gtk4::Label::builder()
.label(format!("{} — {}", dev.display_name, state))
.xalign(0.0)
.margin_start(16)
.build();
 list.append(&row);
 }
 if !any {
 let none = match kind {
 DeviceType::Scanner => tr("Kein Scanner gefunden"),
 DeviceType::Printer => tr("Kein Drucker gefunden"),
 };
 let row = gtk4::Label::builder()
.label(none)
.xalign(0.0)
.margin_start(16)
.build();
 row.add_css_class("dim-label");
 list.append(&row);
 }
 }
 }
 }
}

/// Live "searching" row shown under a section header while the
/// discovery worker is still probing devices.
fn pending_row() -> gtk4::Widget {
 let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 row.set_margin_start(16);
 let spin = gtk4::Spinner::new();
 spin.start();
 row.append(&spin);
 row.append(&gtk4::Label::new(Some(&tr("Suche Geräte…"))));
 row.upcast()
}
