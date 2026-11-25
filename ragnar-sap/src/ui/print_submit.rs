// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — print submission helpers
//
// Shared between the scan-and-print and print-only windows: render
// scanned pages or picked images into a temporary PDF (lopdf writer)
// and hand it
// to CUPS — or keep the file when the destination is the virtual
// "PDF printer".

use gtk4::prelude::*;

use crate::i18n::tr;
use crate::image::pnm::Rgb8Image;
use crate::pdf::{create, manager::PdfMetadata};

/// Temporary PDF path for one print job (unique per call).
fn temp_pdf_path() -> String {
 let mut p = std::env::temp_dir();
 p.push(format!("ragnar-sap-printjob-{}-{}.pdf", std::process::id(), now_nanos()));
 p.to_string_lossy().to_string()
}

/// Monotonic-ish suffix so parallel jobs never collide.
fn now_nanos() -> u128 {
 std::time::SystemTime::now()
.duration_since(std::time::UNIX_EPOCH)
.map(|d| d.as_nanos())
.unwrap_or(0)
}

/// Render `pages` to a temp PDF and submit it to `dest` via CUPS.
/// When `dest` names the virtual PDF printer ("PDF" / file: URI) the
/// PDF is kept and its path reported, matching the design.
pub fn submit_pages(pages: &[Rgb8Image], dpi: i32, dest: &str, copies: i32) -> String {
 let path = temp_pdf_path();
 let meta = PdfMetadata::default();
 if !create::create_pdf_from_images(pages, &path, &meta, dpi) {
 return tr("PDF-Erzeugung fehlgeschlagen.");
 }
 if dest.eq_ignore_ascii_case("pdf") || dest.starts_with("file:") {
 return format!("{} {}", tr("Gespeichert unter"), path);
 }
 let mut opts = Vec::new();
 if copies > 1 {
 opts.push(("copies".to_string(), copies.to_string()));
 }
 let result = crate::cups::job::submit(dest, &path, "ragnar-sap", &opts);
 let _ = std::fs::remove_file(&path); // CUPS already consumed it
 match result {
 Ok(job) => format!("{} #{}", tr("Druckauftrag"), job),
 Err(e) => format!("{}: {}", tr("Druckfehler"), e.0),
 }
}

/// Label + widget row.
pub(super) fn labeled(label: &str, widget: &impl IsA<gtk4::Widget>) -> gtk4::Box {
 let r = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 r.append(&gtk4::Label::builder().label(label).xalign(0.0).width_chars(12).build());
 widget.set_hexpand(true);
 r.append(widget);
 r
}

/// Render the (possibly redacted)
/// preview pages into `dest` at the chosen quality dpi.
pub(super) fn write_document_pdf(pages: &[Rgb8Image], dest: &str, dpi: i32) -> bool {
 crate::pdf::create::create_pdf_from_images(pages, dest, &crate::pdf::manager::PdfMetadata::default(), dpi)
}

/// Submit to CUPS with the full option set. Source PDFs without any
/// redactions go verbatim — saves the raster roundtrip, like the 
/// file export. With redactions the burned-in pages must be
/// rendered instead.
pub(super) fn submit_document(src: &str, pages: &[Rgb8Image], redacted: bool,
 dest: &str, dpi: i32, opts: &[(String, String)]) -> String {
 let file;
 let mut tmp = String::new();
 if !redacted && src.to_lowercase().ends_with(".pdf") {
 file = src.to_string();
 } else {
 tmp = std::env::temp_dir()
.join(format!("ragnar-sap-po-{}.pdf", std::process::id()))
.to_string_lossy().to_string();
 if !write_document_pdf(pages, &tmp, dpi) {
 return tr("Druckauftrag konnte nicht vorbereitet werden.");
 }
 file = tmp.clone();
 }
 let result = crate::cups::job::submit(dest, &file, "ragnar-sap", opts);
 if !tmp.is_empty() {
 let _ = std::fs::remove_file(&tmp);
 }
 match result {
 Ok(job) => format!("{} #{}", tr("Druckauftrag"), job),
 Err(e) => format!("{}: {}", tr("Druckfehler"), e.0),
 }
}
