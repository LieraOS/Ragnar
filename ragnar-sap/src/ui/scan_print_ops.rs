// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan-and-print window handlers
//
// scan trigger + finish
// (pipeline: ui auto-crop → page append → optional multi-page ask),
// page navigation with per-page redaction/adjustment persistence,
// and print: burn each page's redactions into its pixels, write PDF
// (PDF Printer → save dialog) or submit a temp PDF to CUPS.

use gtk4::prelude::*;
use gtk4::gio;

use crate::core::preferences::{Preferences, ScannerDefaults};
use crate::i18n::tr;
use crate::image::{color, empty_page, pnm, pnm::Rgb8Image, redact, ui_crop};
use crate::sane::session::{ScanOptions, Session};
use super::{print_submit, worker};
use super::scan_print::ScanUi;
use super::scan_print_widgets::{map_color_mode, parse_dpi, source_value};
use std::rc::Rc;
use gtk4::glib;

/// Kick off one scan on a worker thread (a worker thread::run ).
pub fn start_scan(ui: &Rc<ScanUi>) {
 let dev = ui.last_device.borrow().clone();
 if dev.is_empty() {
 ui.info.set_label(&tr("Kein Scanner ausgewählt oder Scan läuft bereits."));
 return;
 }
 let opts = ScanOptions {
 resolution_dpi: parse_dpi(&ui.res_cb.text()),
 color_mode: map_color_mode(&ui.color_cb.text()),
 source: source_value(&ui.source_cb.text()),
 };
 // Remember the device + the options actually used — the next
 // window open restores them (same file ragnar-settings writes).
 {
 let mut prefs = Preferences::load();
 prefs.set_last_scanner(&dev);
 prefs.set_scanner_defaults(&dev, ScannerDefaults {
 resolution_dpi: opts.resolution_dpi,
 color_mode: opts.color_mode.clone(),
 source: opts.source.clone(),
 });
 prefs.save_to_file(&crate::core::preferences::config_path().to_string_lossy());
 }
 ui.scan_btn.set_sensitive(false);
 ui.info.set_label(&tr("Scanne…"));

 let ui2 = ui.clone();
 worker::spawn(move || -> Result<Vec<u8>, String> {
 let mut s = Session::open(&dev)
.map_err(|e| format!("{}: {}", tr("Scanner kann nicht geöffnet werden"), e))?;
 s.apply_options(&opts);
 s.full_scan()
.filter(|b| !b.is_empty())
.ok_or_else(|| tr("Scan fehlgeschlagen. Scanner-Verbindung und SANE-Setup prüfen."))
 },
 move |res| scan_finished(&ui2, res),
 );
}

/// Scan completion: decode, apply the enabled pipeline stages,
/// append the page and — in multi-page mode — ask for another page.
fn scan_finished(ui: &Rc<ScanUi>, result: Result<Vec<u8>, String>) {
 ui.scan_btn.set_sensitive(true);
 let bytes = match result {
 Ok(b) => b,
 Err(e) => {
 ui.info.set_label(&e);
 return;
 }
 };
 let Some(mut img) = pnm::decode(&bytes) else {
 ui.info.set_label(&tr("Scan lieferte ein nicht unterstütztes Format."));
 return;
 };

 // Pipeline stages gated by the advanced-options checkboxes —
 // in the design only auto-crop was actually read, the rest
 // now runs for real (the backends were ported already).
 if ui.crop_cb.is_active() {
 img = ui_crop::auto_crop_ui(&img);
 }
 if ui.rotate_cb.is_active() {
 let rot = crate::image::deskew::detect_rotation(&img);
 img = crate::image::crop::rotate(&img, rot);
 }
 if ui.optimize_cb.is_active() {
 let params = color::analyse(&img);
 img = color::apply(&img, &params);
 }
 if ui.empty_cb.is_active() && empty_page::is_page_empty(&img, 0.95) {
 ui.info.set_label(&tr("Leere Seite verworfen."));
 return;
 }

 {
 let mut s = ui.state.borrow_mut();
 s.pages.push(img);
 s.redactions.push(Vec::new());
 s.adjusts.push((100, 0));
 s.current = s.pages.len() - 1;
 }
 ui.print_btn.set_sensitive(true);
 // Bind first — a temporary .borrow() would live through the
 // show_page call and collide with its borrow_mut (panic → abort).
 let cur = ui.state.borrow().current;
 show_page(ui, cur);

 if ui.multi_cb.is_active() {
 ask_another_page(ui);
 } else {
 let n = ui.state.borrow().pages.len();
 ui.info.set_label(&tr("Scan abgeschlossen (%1 Seite(n)). Zum Fortfahren Druck drücken.").replace("%1", &n.to_string()));
 }
}

/// The "Page %1 scanned. Scan another page?" prompt.
fn ask_another_page(ui: &Rc<ScanUi>) {
 let n = ui.state.borrow().pages.len();
 let dialog = gtk4::AlertDialog::builder()
.modal(true)
.message(tr("Mehrseiten-Scan"))
.detail(tr("Seite %1 gescannt. Eine weitere Seite scannen?").replace("%1", &n.to_string()))
.buttons([tr("Nein"), tr("Ja")])
.cancel_button(0)
.default_button(1)
.build();
 let ui2 = ui.clone();
 let win = ui.window.clone();
 glib::spawn_future_local(async move {
 if dialog.choose_future(Some(&win)).await.unwrap_or(0) == 1 {
 start_scan(&ui2);
 } else {
 let n = ui2.state.borrow().pages.len();
 ui2.info.set_label(&tr("Scan abgeschlossen (%1 Seite(n)). Zum Fortfahren Druck drücken.").replace("%1", &n.to_string()));
 }
 });
}

/// Persist the preview's current redactions + slider values, then
/// display page `idx` with its own stored state (page display ).
fn show_page(ui: &Rc<ScanUi>, idx: usize) {
 // Store the outgoing page's UI state first (did this in
 // page display before switching).
 {
 let mut s = ui.state.borrow_mut();
 if !s.pages.is_empty() {
 let cur = s.current;
 s.redactions[cur] = ui.preview.redaction_rects();
 s.adjusts[cur] = (ui.contrast.value() as i32, ui.bright.value() as i32);
 s.current = idx.min(s.pages.len() - 1);
 }
 }
 let (img, rects, (c, b)) = {
 let s = ui.state.borrow();
 let i = s.current;
 (s.pages[i].clone(), s.redactions[i].clone(), s.adjusts[i])
 };
 // Restore this page's slider values before showing (blockSignals
 // equivalent: set_value triggers our handler which writes back
 // the same values — harmless and idempotent).
 ui.contrast.set_value(c as f64);
 ui.bright.set_value(b as f64);
 let adjusted = color::adjust_brightness_contrast(&img, b, c);
 ui.preview.set_image(&adjusted, false);
 ui.preview.set_redactions(rects);
 let n = ui.state.borrow().pages.len();
 ui.page_lbl.set_label(&format!("{} / {}", idx + 1, n));
}

/// Prev/next page navigation.
pub fn step(ui: &Rc<ScanUi>, dir: i64) {
 let n = ui.state.borrow().pages.len();
 if n == 0 {
 return;
 }
 let cur = ui.state.borrow().current as i64;
 show_page(ui, (cur + dir).clamp(0, n as i64 - 1) as usize);
}

/// Print: apply every page's adjustments + redactions at full
/// resolution, then save as PDF (PDF Printer) or submit to CUPS.
pub fn print(ui: &Rc<ScanUi>) {
 if ui.state.borrow().pages.is_empty() {
 ui.info.set_label(&tr("Noch nichts gescannt."));
 return;
 }
 // Persist current page's on-screen state first.
 {
 let mut s = ui.state.borrow_mut();
 let cur = s.current;
 s.redactions[cur] = ui.preview.redaction_rects();
 s.adjusts[cur] = (ui.contrast.value() as i32, ui.bright.value() as i32);
 }
 let output: Vec<Rgb8Image> = {
 let s = ui.state.borrow();
 (0..s.pages.len())
.map(|i| {
 let (c, b) = s.adjusts[i];
 let adjusted = color::adjust_brightness_contrast(&s.pages[i], b, c);
 redact::apply_redactions(&adjusted, &s.redactions[i])
 })
.collect()
 };
 let dpi = parse_dpi(&ui.res_cb.text());
 let sel = ui.printer_cb.dd.selected() as usize;
 let format = ui.format_cb.text().to_lowercase();

 if sel == 0 {
 // Virtual PDF Printer: ask for a destination and write the
 // chosen format (PDF document or per-page PNG/JPEG).
 let ext = if format == "pdf" { "pdf" } else { format.as_str() };
 let dialog = gtk4::FileDialog::builder()
.title(tr("Gescanntes Dokument speichern"))
.modal(true)
.initial_name(format!("scan.{ext}"))
.build();
 let win = ui.window.clone();
 let info = ui.info.clone();
 dialog.save(Some(&win), gio::Cancellable::NONE, move |res| {
 let Ok(file) = res else { return };
 let Some(path) = file.path() else { return };
 let p = path.to_string_lossy().to_string();
 let info2 = info.clone();
 worker::spawn(move || save_pages(&output, &p, &format, dpi),
 move |msg| info2.set_label(&msg),
 );
 });
 return;
 }

 let Some(dest) = ui.printer_ids.get(sel - 1).cloned() else {
 ui.info.set_label(&tr("Kein Drucker ausgewählt."));
 return;
 };
 {
 let mut prefs = Preferences::load();
 prefs.set_last_printer(&dest);
 prefs.save_to_file(&crate::core::preferences::config_path().to_string_lossy());
 }
 ui.info.set_label(&tr("Sende an Drucker…"));
 let info = ui.info.clone();
 worker::spawn(move || print_submit::submit_pages(&output, dpi, &dest, 1),
 move |msg| info.set_label(&msg),
 );
}

/// Write `pages` to `path` in the chosen format: one PDF document,
/// or per-page image files (`stem-1.png`,.) for PNG/JPEG — the
/// image crate encoders make format combo actually work.
fn save_pages(pages: &[Rgb8Image], path: &str, format: &str, dpi: i32) -> String {
 if format == "pdf" {
 let ok = crate::pdf::create::create_pdf_from_images(pages, path, &crate::pdf::manager::PdfMetadata::default(), dpi);
 return if ok { tr("PDF gespeichert.") } else { tr("PDF konnte nicht geschrieben werden.") };
 }
 let stem = std::path::Path::new(path);
 let dir = stem.parent().unwrap_or(std::path::Path::new("."));
 let base = stem.file_stem().and_then(|s| s.to_str()).unwrap_or("scan");
 for (i, page) in pages.iter().enumerate() {
 let name = if pages.len() == 1 {
 path.to_string()
 } else {
 dir.join(format!("{base}-{}.{format}", i + 1)).to_string_lossy().to_string()
 };
 let fmt = if format == "jpeg" || format == "jpg" {
 image::ImageFormat::Jpeg
 } else {
 image::ImageFormat::Png
 };
 if image::save_buffer_with_format(&name, &page.data, page.width as u32, page.height as u32,
 image::ExtendedColorType::Rgb8, fmt,
 ).is_err() {
 return tr("Bild konnte nicht geschrieben werden.");
 }
 }
 tr("Gespeichert.")
}
