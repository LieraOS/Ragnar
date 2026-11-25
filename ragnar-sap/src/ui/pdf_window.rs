// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — PDF management window
//
// combine, compress (ghostscript),
// OCR (tesseract), TIFF export (pdftoppm) and page-count info on
// user-picked PDF files. Every operation runs on a worker thread and
// reports its result in the status line.

use gtk4::prelude::*;
use gtk4::gio;

use crate::i18n::tr;
use crate::pdf::manager::{self, CompressionSettings};
use super::worker;

/// Open the PDF management window as a modal child of `parent`.
pub fn present_pdf_window(parent: &impl IsA<gtk4::Widget>) {
 let window = gtk4::Window::builder()
.title(tr("PDF-Verwaltung"))
.transient_for(&parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok()).unwrap_or_default())
.modal(true)
.default_width(560)
.build();

 let header = gtk4::HeaderBar::new();
 window.set_titlebar(Some(&header));

 let root = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
 root.set_margin_top(14);
 root.set_margin_bottom(14);
 root.set_margin_start(14);
 root.set_margin_end(14);

 // Selected files list.
 let list = gtk4::ListBox::new();
 list.set_selection_mode(gtk4::SelectionMode::None);
 let scroll = gtk4::ScrolledWindow::builder()
.child(&list)
.min_content_height(120)
.build();
 root.append(&scroll);

 let add_btn = gtk4::Button::with_label(&tr("PDF hinzufügen…"));
 root.append(&add_btn);

 // Operation buttons.
 let ops = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 ops.set_homogeneous(true);
 let btn_combine = gtk4::Button::with_label(&tr("Kombinieren"));
 let btn_compress = gtk4::Button::with_label(&tr("Komprimieren"));
 let btn_ocr = gtk4::Button::with_label(&tr("OCR"));
 let btn_tiff = gtk4::Button::with_label(&tr("TIFF-Export"));
 for b in [&btn_combine, &btn_compress, &btn_ocr, &btn_tiff] {
 ops.append(b);
 }
 root.append(&ops);

 let status = gtk4::Label::builder().label(tr("Bereit.")).xalign(0.0).wrap(true).build();
 root.append(&status);
 window.set_child(Some(&root));

 let files = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));

 add_btn.connect_clicked({
 let window = window.clone();
 let files = files.clone();
 let list = list.clone();
 move |_| {
 let dialog = gtk4::FileDialog::builder()
.title(tr("PDF wählen"))
.modal(true)
.build();
 let filter = gtk4::FileFilter::new();
 filter.add_mime_type("application/pdf");
 let filters = gio::ListStore::new::<gtk4::FileFilter>();
 filters.append(&filter);
 dialog.set_filters(Some(&filters));
 let files = files.clone();
 let list = list.clone();
 dialog.open_multiple(Some(&window), gio::Cancellable::NONE, move |res| {
 let Ok(model) = res else { return };
 for i in 0..model.n_items() {
 let Some(item) = model.item(i).and_downcast::<gio::File>() else { continue };
 if let Some(path) = item.path() {
 let p = path.to_string_lossy().to_string();
 files.borrow_mut().push(p.clone());
 list.append(&gtk4::Label::builder()
.label(&p).xalign(0.0)
.ellipsize(gtk4::pango::EllipsizeMode::Middle)
.build());
 }
 }
 });
 }
 });

 // Each button runs its manager() op on a worker thread.
 btn_combine.connect_clicked({
 let status = status.clone();
 let files = files.clone();
 let window = window.clone();
 move |_| {
 let inputs = files.borrow().clone();
 if inputs.len() < 2 {
 status.set_label(&tr("Mindestens zwei PDFs nötig."));
 return;
 }
 let status_c = status.clone();
 save_dialog(&window, &tr("Kombinierte PDF speichern"), move |out| {
 let status = status_c;
 worker::spawn(move || {
 manager::combine_pdfs(&inputs, &out, &manager::PdfMetadata::default())
 },
 move |ok| status.set_label(&result_msg(ok)),
 );
 });
 }
 });

 btn_compress.connect_clicked({
 let status = status.clone();
 let files = files.clone();
 let window = window.clone();
 move |_| {
 let Some(input) = files.borrow().first().cloned() else {
 status.set_label(&tr("Keine Datei gewählt."));
 return;
 };
 let status_c = status.clone();
 save_dialog(&window, &tr("Komprimierte PDF speichern"), move |out| {
 let status = status_c;
 worker::spawn(move || manager::compress_pdf(&input, &out, &CompressionSettings { image_resolution: 150 }),
 move |ok| status.set_label(&result_msg(ok)),
 );
 });
 }
 });

 btn_ocr.connect_clicked({
 let status = status.clone();
 let files = files.clone();
 let window = window.clone();
 move |_| {
 let Some(input) = files.borrow().first().cloned() else {
 status.set_label(&tr("Keine Datei gewählt."));
 return;
 };
 let status_c = status.clone();
 save_dialog(&window, &tr("OCR-PDF speichern"), move |out| {
 let status = status_c;
 worker::spawn(move || manager::perform_ocr(&input, &out, "deu"),
 move |ok| status.set_label(&result_msg(ok)),
 );
 });
 }
 });

 btn_tiff.connect_clicked({
 let status = status.clone();
 let files = files.clone();
 move |_| {
 let Some(input) = files.borrow().first().cloned() else {
 status.set_label(&tr("Keine Datei gewählt."));
 return;
 };
 status.set_label(&tr("Exportiere…"));
 let status2 = status.clone();
 worker::spawn(move || {
 let out = input.trim_end_matches(".pdf").to_string() + ".tiff";
 manager::export_to_tiff(&input, &out, 1, -1)
 },
 move |ok| status2.set_label(&result_msg(ok)),
 );
 }
 });

 window.present();
}

/// Status text for a boolean manager() result.
fn result_msg(ok: bool) -> String {
 if ok { tr("Fertig.") } else { tr("Vorgang fehlgeschlagen.") }
}

/// Ask for an output path, then invoke `cb` with it.
fn save_dialog(window: &gtk4::Window, title: &str, cb: impl FnOnce(String) + 'static) {
 let dialog = gtk4::FileDialog::builder()
.title(title)
.modal(true)
.initial_name("output.pdf")
.build();
 dialog.save(Some(window), gio::Cancellable::NONE, move |res| {
 if let Ok(file) = res {
 if let Some(path) = file.path() {
 cb(path.to_string_lossy().to_string());
 }
 }
 });
}
