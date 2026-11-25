// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — print-only window
//
// Faithful file selection (all /
// text / images / html / pdf), a real document preview with zoom
// buttons and page navigation, print settings (printer incl. the
// virtual PDF Printer, copies, paper size, quality, color mode),
// redaction on the preview (rubber-band boxes burned into the page
// pixels — text-span approach maps onto the same
// visual contract), and submission to CUPS with media /
// print-quality / color-mode options.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::gio;

use crate::core::device_info::DeviceType;
use crate::core::registry::DeviceRegistry;
use crate::i18n::tr;
use crate::image::{pnm::Rgb8Image, redact};
use super::{doc_render, preview::ScanPreview, print_submit, scan_print_widgets::UiCombo, worker};

/// Shared window state.
struct PoState {
 file: Option<String>,
 pages: Vec<Rgb8Image>,
 redactions: Vec<Vec<redact::Rect>>,
 current: usize,
}

/// Paper size combo entries: (label, CUPS media name).
const MEDIA: &[(&str, &str)] = &[
 ("A4", "iso_a4_210x297"),
 ("A3", "iso_a3_297x420"),
 ("A5", "iso_a5_148x210"),
 ("Letter", "na_letter_8..5x11in"),
 ("Legal", "na_legal_8.5x14in"),
];

/// Quality combo entries: (label, dpi for rendering, CUPS keyword).
const QUALITY: &[(&str, i32, &str)] = &[
 ("Draft (150 DPI)", 150, "draft"),
 ("Normal (300 DPI)", 300, "normal"),
 ("High (600 DPI)", 600, "high"),
];

/// Open the print-only window as a modal child of `parent`.
pub fn present_print_only(parent: &impl IsA<gtk4::Widget>, registry: Rc<DeviceRegistry>) {
 let window = gtk4::Window::builder()
.title(tr("Druck-Workflow"))
.transient_for(&parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok()).unwrap_or_default())
.modal(true)
.default_width(1000)
.default_height(680)
.build();
 window.set_titlebar(Some(&gtk4::HeaderBar::new()));

 let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
 paned.set_position(360);
 paned.set_shrink_start_child(false);

 // ================= left: controls ==============================
 let controls = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
 controls.set_margin_top(10);
 controls.set_margin_bottom(10);
 controls.set_margin_start(10);
 controls.set_margin_end(10);

 // File selection group.
 let file_frame = gtk4::Frame::new(Some(&tr("Dateiauswahl")));
 let file_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
 file_box.set_margin_top(6);
 file_box.set_margin_bottom(8);
 file_box.set_margin_start(8);
 file_box.set_margin_end(8);
 let file_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 file_row.append(&gtk4::Label::builder().label(tr("Datei:")).xalign(0.0).build());
 let file_lbl = gtk4::Label::builder()
.label(tr("Keine Datei ausgewählt"))
.xalign(0.0)
.hexpand(true)
.ellipsize(gtk4::pango::EllipsizeMode::Middle)
.build();
 let browse = gtk4::Button::with_label(&tr("Durchsuchen…"));
 file_row.append(&file_lbl);
 file_row.append(&browse);
 file_box.append(&file_row);
 file_frame.set_child(Some(&file_box));
 controls.append(&file_frame);

 // Print settings group.
 let set_frame = gtk4::Frame::new(Some(&tr("Druckeinstellungen")));
 let set_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
 set_box.set_margin_top(6);
 set_box.set_margin_bottom(8);
 set_box.set_margin_start(8);
 set_box.set_margin_end(8);

 let printer_ids: Vec<String> = registry.list(DeviceType::Printer).iter().map(|d| d.id.clone()).collect();
 let mut printer_items: Vec<String> = vec![tr("PDF Drucker")];
 printer_items.extend(registry.list(DeviceType::Printer).iter().map(|d| d.display_name.clone()));
 let printer_cb = UiCombo::new(&printer_items.iter().map(|s| s.as_str()).collect::<Vec<_>>());
 let media_cb = UiCombo::new(&MEDIA.iter().map(|m| m.0).collect::<Vec<_>>());
 let quality_cb = UiCombo::new(&QUALITY.iter().map(|q| q.0).collect::<Vec<_>>());
 quality_cb.dd.set_selected(1);
 let colormode_cb = UiCombo::new(&[tr("Farbe"), tr("Graustufen"), tr("Schwarz & Weiß")]);
 let copies = gtk4::SpinButton::with_range(1.0, 99.0, 1.0);
 copies.set_hexpand(true);

 set_box.append(&print_submit::labeled(&tr("Drucker:"), &printer_cb.dd));
 set_box.append(&print_submit::labeled(&tr("Kopien:"), &copies));
 set_box.append(&print_submit::labeled(&tr("Papierformat:"), &media_cb.dd));
 set_box.append(&print_submit::labeled(&tr("Qualität:"), &quality_cb.dd));
 set_box.append(&print_submit::labeled(&tr("Farbmodus:"), &colormode_cb.dd));
 set_frame.set_child(Some(&set_box));
 controls.append(&set_frame);

 // Redaction group.
 let red_frame = gtk4::Frame::new(Some(&tr("Schwärzung")));
 let red_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
 red_box.set_margin_top(6);
 red_box.set_margin_bottom(8);
 red_box.set_margin_start(8);
 red_box.set_margin_end(8);
 red_box.append(&gtk4::Label::builder()
.label(tr("Schwärzen aktivieren und Boxen in der Vorschau aufziehen:"))
.xalign(0.0)
.wrap(true)
.build());
 let redact_btn = gtk4::ToggleButton::with_label(&tr("🔒 Schwärzen"));
 red_box.append(&redact_btn);
 red_frame.set_child(Some(&red_box));
 controls.append(&red_frame);

 let print_btn = gtk4::Button::with_label(&tr("Drucken"));
 print_btn.add_css_class("suggested-action");
 print_btn.set_sensitive(false);
 controls.append(&print_btn);

 let info = gtk4::Label::builder()
.label(tr("Datei zum Drucken wählen."))
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
 head.append(&gtk4::Label::builder().label(format!("<b>{}</b>", tr("Druckvorschau"))).use_markup(true).build());
 let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
 spacer.set_hexpand(true);
 head.append(&spacer);
 let prev_btn = gtk4::Button::with_label("◀ Zurück");
 let page_lbl = gtk4::Label::new(Some(&tr("Seite 0 von 0")));
 let next_btn = gtk4::Button::with_label("Weiter ▶");
 head.append(&prev_btn);
 head.append(&page_lbl);
 head.append(&next_btn);
 preview_box.append(&head);
 let preview = ScanPreview::new();
 preview_box.append(&preview.area);
 paned.set_end_child(Some(&preview_box));
 window.set_child(Some(&paned));

 // ================= state + wiring ==============================
 let state = Rc::new(RefCell::new(PoState {
 file: None,
 pages: Vec::new(),
 redactions: Vec::new(),
 current: 0,
 }));

 redact_btn.connect_toggled({
 let preview = preview.clone();
 move |b| preview.set_redact_mode(b.is_active())
 });

 browse.connect_clicked({
 let window = window.clone();
 let file_lbl = file_lbl.clone();
 let info = info.clone();
 let state = state.clone();
 let preview = preview.clone();
 let page_lbl = page_lbl.clone();
 let print_btn = print_btn.clone();
 move |_| {
 let dialog = gtk4::FileDialog::builder()
.title(tr("Datei zum Drucken wählen"))
.modal(true)
.build();
 let filter = gtk4::FileFilter::new();
 for mime in ["application/pdf", "text/plain", "text/markdown",
 "text/html", "image/png", "image/jpeg"] {
 filter.add_mime_type(mime);
 }
 let filters = gio::ListStore::new::<gtk4::FileFilter>();
 filters.append(&filter);
 dialog.set_filters(Some(&filters));
 let file_lbl = file_lbl.clone();
 let info = info.clone();
 let state = state.clone();
 let preview = preview.clone();
 let page_lbl = page_lbl.clone();
 let print_btn = print_btn.clone();
 dialog.open(Some(&window), gio::Cancellable::NONE, move |res| {
 let Ok(file) = res else { return };
 let Some(path) = file.path() else { return };
 let p = path.to_string_lossy().to_string();
 file_lbl.set_label(&p);
 info.set_label(&tr("Rendere Vorschau…"));
 let info2 = info.clone();
 let state2 = state.clone();
 let preview2 = preview.clone();
 let page_lbl2 = page_lbl.clone();
 let print_btn2 = print_btn.clone();
 state.borrow_mut().file = Some(p.clone());
 worker::spawn(move || doc_render::render(&p),
 move |pages| {
 if pages.is_empty() {
 info2.set_label(&tr("Datei konnte nicht geladen werden."));
 return;
 }
 let n = pages.len();
 {
 let mut s = state2.borrow_mut();
 s.redactions = vec![Vec::new(); n];
 s.pages = pages;
 s.current = 0;
 }
 let img = state2.borrow().pages[0].clone();
 preview2.set_image(&img, false);
 preview2.set_redactions(Vec::new());
 page_lbl2.set_label(&tr("Seite 1 von %1").replace("%1", &n.to_string()));
 print_btn2.set_sensitive(true);
 info2.set_label(&format!("{} {}", n, tr("Seite(n) geladen.")));
 },
 );
 });
 }
 });

 // Page navigation — persist current redactions before switching.
 let goto = {
 let state = state.clone();
 let preview = preview.clone();
 let page_lbl = page_lbl.clone();
 move |dir: i64| {
 let mut s = state.borrow_mut();
 if s.pages.is_empty() {
 return;
 }
 let cur = s.current;
 s.redactions[cur] = preview.redaction_rects();
 let next = (s.current as i64 + dir).clamp(0, s.pages.len() as i64 - 1) as usize;
 s.current = next;
 let img = s.pages[next].clone();
 let rects = s.redactions[next].clone();
 let n = s.pages.len();
 drop(s);
 preview.set_image(&img, false);
 preview.set_redactions(rects);
 page_lbl.set_label(&tr("Seite %1 von %2")
.replace("%1", &(next + 1).to_string())
.replace("%2", &n.to_string()));
 }
 };
 let goto = Rc::new(goto);
 prev_btn.connect_clicked({ let g = goto.clone(); move |_| g(-1) });
 next_btn.connect_clicked({ let g = goto.clone(); move |_| g(1) });

 // Print — document export.
 print_btn.connect_clicked({
 let window = window.clone();
 let info = info.clone();
 let state = state.clone();
 let preview = preview.clone();
 let printer_cb = printer_cb.dd.clone();
 let printer_ids = printer_ids.clone();
 let media_cb = media_cb.dd.clone();
 let quality_cb = quality_cb.dd.clone();
 let colormode_cb = colormode_cb.dd.clone();
 let copies = copies.clone();
 move |_| {
 let Some(path) = state.borrow().file.clone() else {
 info.set_label(&tr("Keine Datei ausgewählt."));
 return;
 };
 // Finalize pages with their redactions burned in.
 let (output, any_red) = {
 let mut s = state.borrow_mut();
 if s.pages.is_empty() {
 info.set_label(&tr("Keine Datei ausgewählt."));
 return;
 }
 let cur = s.current;
 s.redactions[cur] = preview.redaction_rects();
 let any = s.redactions.iter().any(|r| !r.is_empty());
 let pages: Vec<Rgb8Image> = (0..s.pages.len())
.map(|i| redact::apply_redactions(&s.pages[i], &s.redactions[i]))
.collect();
 (pages, any)
 };
 let sel = printer_cb.selected() as usize;
 let media = MEDIA[media_cb.selected() as usize % MEDIA.len()].1;
 let (qdpi, qkey) = {
 let q = QUALITY[quality_cb.selected() as usize % QUALITY.len()];
 (q.1, q.2)
 };
 let monochrome = colormode_cb.selected() != 0;
 let n_copies = copies.value() as i32;

 if sel == 0 {
 // PDF Printer → save dialog, honoring the settings.
 let dialog = gtk4::FileDialog::builder()
.title(tr("Dokument als PDF speichern"))
.modal(true)
.initial_name("output.pdf")
.build();
 let win = window.clone();
 let info2 = info.clone();
 dialog.save(Some(&win), gio::Cancellable::NONE, move |res| {
 let Ok(f) = res else { return };
 let Some(p) = f.path() else { return };
 let dest = p.to_string_lossy().to_string();
 let info3 = info2.clone();
 worker::spawn(move || print_submit::write_document_pdf(&output, &dest, qdpi),
 move |ok| info3.set_label(&if ok {
 tr("PDF gespeichert.")
 } else {
 tr("PDF konnte nicht geschrieben werden.")
 }),
 );
 });
 return;
 }

 let Some(dest) = printer_ids.get(sel - 1).cloned() else {
 info.set_label(&tr("Kein Drucker ausgewählt."));
 return;
 };
 // Options exactly like submission.
 let mut opts = vec![
 ("media".to_string(), media.to_string()),
 ("print-quality".to_string(), qkey.to_string()),
 ];
 if monochrome {
 opts.push(("print-color-mode".to_string(), "monochrome".to_string()));
 }
 if n_copies > 1 {
 opts.push(("copies".to_string(), n_copies.to_string()));
 }
 info.set_label(&tr("Sende an Drucker…"));
 let info2 = info.clone();
 worker::spawn(move || print_submit::submit_document(&path, &output, any_red, &dest, qdpi, &opts),
 move |msg| info2.set_label(&msg),
 );
 }
 });

 window.present();
}
