// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan-and-print window
//
// Faithful device group (scanner
// via SANE, printer via CUPS + the virtual "PDF Printer"), scan
// profile presets (Documents/Letters/Photos), scan settings
// repopulated from live SANE capabilities, collapsible advanced
// options (multi-page, auto-crop, auto-rotate, empty-page filter,
// color optimization, contrast/brightness sliders), the redact-
// capable preview with page navigation, and the print path that
// burns every page's own redactions into the pixels before writing
// the PDF / submitting to CUPS.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::core::device_info::DeviceType;
use crate::core::registry::DeviceRegistry;
use crate::i18n::tr;
use crate::image::{color, pnm::Rgb8Image, redact};
use crate::sane::session::Session;
use super::scan_print_ops::{print, start_scan, step};
use crate::core::preferences::Preferences;
use super::scan_print_widgets::{
 checked, group_box, group_inner, row, select_containing, select_mode, select_source,
 share, slider, UiCombo,
};
use super::{preview::ScanPreview, worker};

/// Per-page state — keeps pages, per-page redaction lists and per-page adjustments.
pub(super) struct PageState {
 pub pages: Vec<Rgb8Image>,
 pub redactions: Vec<Vec<redact::Rect>>,
 /// (contrast_pct, brightness) per page.
 pub adjusts: Vec<(i32, i32)>,
 pub current: usize,
}

/// Scan profile preset values (presets).
struct Profile {
 dpi: i32,
 color: &'static str,
 source: &'static str,
 multi: bool,
 crop: bool,
 rotate: bool,
 empty: bool,
 optimize: bool,
}

fn preset_profile(name: &str) -> Profile {
 match name {
 "Letters" => Profile { dpi: 300, color: "Gray", source: "ADF", multi: false,
 crop: true, rotate: true, empty: true, optimize: true },
 "Photos" => Profile { dpi: 600, color: "Color", source: "Flatbed", multi: false,
 crop: false, rotate: false, empty: false, optimize: true },
 _ => Profile { dpi: 300, color: "Lineart", source: "ADF", multi: true,
 crop: true, rotate: true, empty: true, optimize: false },
 }
}

/// Open the scan-and-print window as a modal child of `parent`.
pub fn present_scan_print(parent: &impl IsA<gtk4::Widget>, registry: Rc<DeviceRegistry>) {
 let window = gtk4::Window::builder()
.title(tr("Scan- & Druck-Workflow"))
.transient_for(&parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok()).unwrap_or_default())
.modal(true)
.default_width(1024)
.default_height(680)
.build();
 window.set_titlebar(Some(&gtk4::HeaderBar::new()));

 let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
 paned.set_position(380);
 paned.set_shrink_start_child(false);

 // ================= left: controls ==============================
 let controls = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
 controls.set_margin_top(10);
 controls.set_margin_bottom(10);
 controls.set_margin_start(10);
 controls.set_margin_end(10);

 // Device group.
 let dev_group = group_box(&tr("Geräteauswahl"));
 let scanner_ids: Vec<String> = registry.list(DeviceType::Scanner).iter().map(|d| d.id.clone()).collect();
 let scanner_items: Vec<String> = registry.list(DeviceType::Scanner).iter().map(|d| d.display_name.clone()).collect();
 let printer_ids: Vec<String> = registry.list(DeviceType::Printer).iter().map(|d| d.id.clone()).collect();
 let mut printer_items: Vec<String> = vec![tr("PDF Drucker")];
 printer_items.extend(registry.list(DeviceType::Printer).iter().map(|d| d.display_name.clone()));
 let scanner_cb = UiCombo::new(&scanner_items.iter().map(|s| s.as_str()).collect::<Vec<_>>());
 let printer_cb = UiCombo::new(&printer_items.iter().map(|s| s.as_str()).collect::<Vec<_>>());
 let dev_inner = group_inner(&dev_group);
 dev_inner.append(&row(&tr("Drucker:"), &printer_cb.dd));
 dev_inner.append(&row(&tr("Scanner:"), &scanner_cb.dd));
 controls.append(&dev_group);

 // Profile group.
 let prof_group = group_box(&tr("Scan-Profil"));
 let profile_cb = UiCombo::new(&[&tr("Dokumente"), &tr("Briefe"), &tr("Fotos")]);
 group_inner(&prof_group).append(&row(&tr("Profil:"), &profile_cb.dd));
 controls.append(&prof_group);

 // Scan settings — repopulated from device capabilities.
 let set_group = group_box(&tr("Scan-Einstellungen"));
 let res_cb = UiCombo::new(&["150 DPI", "300 DPI", "600 DPI"]);
 res_cb.dd.set_selected(1);
 let color_cb = UiCombo::new(&["Color", "Grayscale", "Black & White"]);
 let format_cb = UiCombo::new(&["PNG", "JPEG", "PDF"]);
 let source_cb = UiCombo::new(&[&tr("Scannerglas"), "ADF (Simplex)", "ADF (Duplex)"]);
 let set_inner = group_inner(&set_group);
 set_inner.append(&row(&tr("Auflösung"), &res_cb.dd));
 set_inner.append(&row(&tr("Farbmodus"), &color_cb.dd));
 set_inner.append(&row(&tr("Format"), &format_cb.dd));
 set_inner.append(&row(&tr("Quelle"), &source_cb.dd));
 controls.append(&set_group);

 // Advanced options — checkable toggle shows/hides the group.
 let adv_toggle = gtk4::ToggleButton::with_label(&tr("⚙ Erweiterte Optionen"));
 controls.append(&adv_toggle);
 let adv_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
 adv_box.set_visible(false);
 let multi_cb = gtk4::CheckButton::with_label(&tr("Mehrseiten-Scan"));
 multi_cb.set_tooltip_text(Some(&tr("Nach jeder Seite wird gefragt, ob eine weitere Seite gescannt werden soll.")));
 let crop_cb = checked(&tr("Auto-Crop"));
 let rotate_cb = checked(&tr("Auto-Rotate"));
 let empty_cb = checked(&tr("Leere Seiten filtern"));
 let optimize_cb = checked(&tr("Farboptimierung"));
 let contrast_sl = slider(50.0, 150.0, 100.0);
 let bright_sl = slider(-50.0, 50.0, 0.0);
 for w in [&multi_cb, &crop_cb, &rotate_cb, &empty_cb, &optimize_cb] {
 adv_box.append(w);
 }
 adv_box.append(&row(&tr("Kontrast:"), &contrast_sl));
 adv_box.append(&row(&tr("Helligkeit:"), &bright_sl));
 controls.append(&adv_box);
 adv_toggle.connect_toggled({
 let adv = adv_box.clone();
 move |b| adv.set_visible(b.is_active())
 });

 let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 btn_row.set_homogeneous(true);
 let scan_btn = gtk4::Button::with_label(&tr("Scannen"));
 scan_btn.add_css_class("suggested-action");
 let print_btn = gtk4::Button::with_label(&tr("Drucken"));
 print_btn.set_sensitive(false);
 btn_row.append(&scan_btn);
 btn_row.append(&print_btn);
 controls.append(&btn_row);

 let info = gtk4::Label::builder()
.label(tr("Scanner und Drucker konfigurieren, dann scannen."))
.xalign(0.0)
.wrap(true)
.build();
 controls.append(&info);
 let scroll = gtk4::ScrolledWindow::builder().child(&controls).propagate_natural_height(true).build();
 paned.set_start_child(Some(&scroll));

 // ================= right: preview ==============================
 let preview_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
 preview_box.set_margin_top(10);
 preview_box.set_margin_bottom(10);
 preview_box.set_margin_start(10);
 preview_box.set_margin_end(10);
 let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 head.append(&gtk4::Label::builder().label(format!("<b>{}</b>", tr("Vorschau"))).use_markup(true).build());
 let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
 spacer.set_hexpand(true);
 head.append(&spacer);
 let redact_btn = gtk4::ToggleButton::with_label(&tr("Schwärzen"));
 redact_btn.set_tooltip_text(Some(&tr("Aktivieren zum Aufziehen von Schwärzungsboxen; deaktivieren zum Verschieben der Vorschau")));
 let prev_btn = gtk4::Button::with_label("◀");
 let page_lbl = gtk4::Label::new(Some("0 / 0"));
 let next_btn = gtk4::Button::with_label("▶");
 let undo_btn = gtk4::Button::with_label(&tr("Rückgängig"));
 let clear_btn = gtk4::Button::with_label(&tr("Entfernen"));
 undo_btn.set_sensitive(false);
 clear_btn.set_sensitive(false);
 head.append(&redact_btn);
 head.append(&prev_btn);
 head.append(&page_lbl);
 head.append(&next_btn);
 head.append(&undo_btn);
 head.append(&clear_btn);
 preview_box.append(&head);
 let preview = ScanPreview::new();
 preview_box.append(&preview.area);
 paned.set_end_child(Some(&preview_box));
 window.set_child(Some(&paned));

 // ================= state + wiring ==============================
 let state = Rc::new(RefCell::new(PageState {
 pages: Vec::new(),
 redactions: Vec::new(),
 adjusts: Vec::new(),
 current: 0,
 }));
 let last_device = Rc::new(RefCell::new(String::new()));

 // Pre-select the last used devices (persisted after each scan)
 // before the notify handlers exist — the explicit probe_caps
 // call below then probes the right device exactly once.
 {
 let p = Preferences::load().get();
 if let Some(i) = scanner_ids.iter().position(|id| id == &p.last_scanner_device) {
 scanner_cb.dd.set_selected(i as u32);
 }
 // printer_items[0] is "PDF Drucker", so ids shift by one.
 if let Some(i) = printer_ids.iter().position(|id| id == &p.last_printer_device) {
 printer_cb.dd.set_selected((i + 1) as u32);
 }
 }

 // Scanner change → fetch capabilities → repopulate combos.
 // probe_caps is also invoked once below — with a single
 // scanner there is no selection change to trigger it.
 scanner_cb.dd.connect_selected_notify({
 let res_cb = share(&res_cb);
 let color_cb = share(&color_cb);
 let source_cb = share(&source_cb);
 let scanner_ids = scanner_ids.clone();
 let last_device = last_device.clone();
 let info2 = info.clone();
 move |dd| probe_caps(dd, &scanner_ids, &res_cb, &color_cb, &source_cb, &last_device, &info2)
 });

 // Initial probe for the pre-selected scanner — otherwise the
 // hardcoded defaults stay until the user changes the combo.
 probe_caps(&scanner_cb.dd, &scanner_ids, &share(&res_cb), &share(&color_cb), &share(&source_cb), &last_device, &info);

 // Profile preset → drive combos + checkboxes (semantic matching
 // like the semantic matcher, because real SANE mode strings
 // differ per device).
 profile_cb.dd.connect_selected_notify({
 let res_cb = share(&res_cb);
 let color_cb = share(&color_cb);
 let source_cb = share(&source_cb);
 let multi = multi_cb.clone();
 let crop = crop_cb.clone();
 let rot = rotate_cb.clone();
 let emp = empty_cb.clone();
 let opt = optimize_cb.clone();
 let profile_cb = profile_cb.dd.clone();
 move |_| {
 let name = profile_cb.selected_item().and_downcast::<gtk4::StringObject>()
.map(|s| s.string().to_string()).unwrap_or_default();
 let key = match name.as_str() {
 n if n == tr("Briefe") => "Letters",
 n if n == tr("Fotos") => "Photos",
 _ => "Documents",
 };
 let p = preset_profile(key);
 select_containing(res_cb.as_ref(), &format!("{} DPI", p.dpi));
 select_containing(color_cb.as_ref(), p.color);
 select_containing(source_cb.as_ref(), p.source);
 multi.set_active(p.multi);
 crop.set_active(p.crop);
 rot.set_active(p.rotate);
 emp.set_active(p.empty);
 opt.set_active(p.optimize);
 }
 });

 // Sliders → store per-page adjustment + refresh preview.
 let apply_adjust = {
 let state = state.clone();
 let preview = preview.clone();
 let contrast = contrast_sl.clone();
 let bright = bright_sl.clone();
 move || {
 let mut s = state.borrow_mut();
 if s.pages.is_empty() {
 return;
 }
 let i = s.current;
 s.adjusts[i] = (contrast.value() as i32, bright.value() as i32);
 let (c, b) = s.adjusts[i];
 let img = color::adjust_brightness_contrast(&s.pages[i], b, c);
 drop(s);
 preview.set_image(&img, true);
 }
 };
 let apply_adjust = Rc::new(apply_adjust);
 contrast_sl.connect_value_changed({ let f = apply_adjust.clone(); move |_| f() });
 bright_sl.connect_value_changed({ let f = apply_adjust.clone(); move |_| f() });

 redact_btn.connect_toggled({
 let preview = preview.clone();
 move |b| preview.set_redact_mode(b.is_active())
 });
 preview.on_redactions_changed({
 let undo = undo_btn.clone();
 let clear = clear_btn.clone();
 move |n| {
 undo.set_sensitive(n > 0);
 clear.set_sensitive(n > 0);
 }
 });
 undo_btn.connect_clicked({
 let preview = preview.clone();
 move |_| preview.undo_redaction()
 });
 clear_btn.connect_clicked({
 let preview = preview.clone();
 move |_| preview.clear_redactions()
 });

 let ui = Rc::new(ScanUi {
 window, state, preview, info, page_lbl,
 print_btn: print_btn.clone(), scan_btn: scan_btn.clone(),
 contrast: contrast_sl, bright: bright_sl,
 multi_cb, crop_cb, rotate_cb, empty_cb, optimize_cb,
 res_cb: share(&res_cb), color_cb: share(&color_cb),
 source_cb: share(&source_cb), format_cb: share(&format_cb),
 printer_cb, printer_ids,
 last_device,
 });

 scan_btn.connect_clicked({ let ui = ui.clone(); move |_| start_scan(&ui) });
 prev_btn.connect_clicked({ let ui = ui.clone(); move |_| step(&ui, -1) });
 next_btn.connect_clicked({ let ui = ui.clone(); move |_| step(&ui, 1) });
 print_btn.connect_clicked({ let ui = ui.clone(); move |_| print(&ui) });

 ui.window.present();
}

/// Open the selected scanner and repopulate DPI/mode/source from
/// its real capabilities. Failures are reported in `info` — silent
/// defaults would hide e.g. missing device permissions.
fn probe_caps(dd: &gtk4::DropDown,
 ids: &[String],
 res_cb: &Rc<UiCombo>,
 color_cb: &Rc<UiCombo>,
 source_cb: &Rc<UiCombo>,
 last_device: &Rc<RefCell<String>>,
 info: &gtk4::Label,
) {
 let Some(dev) = ids.get(dd.selected() as usize).cloned() else { return };
 *last_device.borrow_mut() = dev.clone();
 let res_cb = res_cb.clone();
 let color_cb = color_cb.clone();
 let source_cb = source_cb.clone();
 let info = info.clone();
 let name = dd.selected_item().and_downcast::<gtk4::StringObject>().map(|s| s.string().to_string()).unwrap_or_default();
 let dev_saved = dev.clone();
 worker::spawn(move || Session::open(&dev).map(|s| s.caps.clone()),
 move |caps| match caps {
 Ok(c) => {
 let dpis: Vec<String> = c.dpi_list.iter().map(|d| format!("{d} DPI")).collect();
 res_cb.set_items(&dpis.iter().map(|s| s.as_str()).collect::<Vec<_>>(), 1);
 let modes: Vec<&str> = c.modes.iter().map(|s| s.as_str()).collect();
 color_cb.set_items(&modes, 0);
 let mut srcs = vec![tr("Scannerglas")];
 if c.has_adf { srcs.push("ADF (Simplex)".into()); }
 if c.has_duplex { srcs.push("ADF (Duplex)".into()); }
 source_cb.set_items(&srcs.iter().map(|s| s.as_str()).collect::<Vec<_>>(), 0);
 // Restore the values stored for this device — settings-panel
 // defaults or whatever the user scanned with last time.
 if let Some(d) = Preferences::load().scanner_defaults(&dev_saved) {
 select_containing(res_cb.as_ref(), &format!("{} DPI", d.resolution_dpi));
 select_mode(color_cb.as_ref(), &d.color_mode);
 select_source(source_cb.as_ref(), &d.source);
 }
 }
 Err(_) => info.set_label(&format!("{}: {}", tr("Scanner kann nicht geöffnet werden"), name)),
 });
}

/// Widgets + state needed by the scan/print handlers — one Rc so
/// every closure can reach everything without deep captures.
pub(super) struct ScanUi {
 pub window: gtk4::Window,
 pub state: Rc<RefCell<PageState>>,
 pub preview: ScanPreview,
 pub info: gtk4::Label,
 pub page_lbl: gtk4::Label,
 pub print_btn: gtk4::Button,
 pub scan_btn: gtk4::Button,
 pub contrast: gtk4::Scale,
 pub bright: gtk4::Scale,
 pub multi_cb: gtk4::CheckButton,
 pub crop_cb: gtk4::CheckButton,
 pub rotate_cb: gtk4::CheckButton,
 pub empty_cb: gtk4::CheckButton,
 pub optimize_cb: gtk4::CheckButton,
 pub res_cb: Rc<UiCombo>,
 pub color_cb: Rc<UiCombo>,
 pub source_cb: Rc<UiCombo>,
 pub format_cb: Rc<UiCombo>,
 pub printer_cb: UiCombo,
 pub printer_ids: Vec<String>,
 pub last_device: Rc<RefCell<String>>,
}

