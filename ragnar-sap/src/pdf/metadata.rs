// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — PDF metadata via lopdf.
//
// setMetadata/getMetadata were placeholders ; here
// they are real: the /Info dictionary in the document trailer is
// read and written like exiftool would, without an external tool.

use super::manager::PdfMetadata;
use lopdf::{dictionary, Document, Object};

/// Write metadata into a document's trailer /Info dict.
pub fn write(doc: &mut Document, meta: &PdfMetadata) {
 let info = lopdf::dictionary! {
 "Title" => Object::String(meta.title.clone().into_bytes(), lopdf::StringFormat::Literal),
 "Author" => Object::String(meta.author.clone().into_bytes(), lopdf::StringFormat::Literal),
 "Subject" => Object::String(meta.subject.clone().into_bytes(), lopdf::StringFormat::Literal),
 "Keywords" => Object::String(meta.keywords.clone().into_bytes(), lopdf::StringFormat::Literal),
 "Creator" => Object::String(meta.creator.clone().into_bytes(), lopdf::StringFormat::Literal),
 "Producer" => Object::String(meta.producer.clone().into_bytes(), lopdf::StringFormat::Literal),
 };
 let id = doc.add_object(Object::Dictionary(info));
 doc.trailer.set("Info", id);
}

/// Set metadata on an existing PDF file in place — loads, rewrites
/// /Info, saves to the same path.
pub fn set_on_file(path: &str, meta: &PdfMetadata) -> bool {
 if !super::manager::validate_pdf_path(path) {
 return false;
 }
 let mut doc = match Document::load(path) {
 Ok(d) => d,
 Err(_) => return false,
 };
 write(&mut doc, meta);
 doc.save(path).is_ok()
}

fn read_field(_doc: &Document, info: &lopdf::Dictionary, key: &[u8]) -> String {
 info.get(key)
.ok()
.and_then(|o| match o {
 Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).into_owned()),
 _ => None,
 })
.unwrap_or_default()
}

/// Read /Info from a PDF file — empty strings for absent keys,
/// same shape returns.
pub fn get_from_file(path: &str) -> PdfMetadata {
 let mut meta = PdfMetadata::default();
 if !super::manager::validate_pdf_path(path) {
 return meta;
 }
 let doc = match Document::load(path) {
 Ok(d) => d,
 Err(_) => return meta,
 };
 let info = doc
.trailer
.get(b"Info")
.ok()
.and_then(|o| o.as_reference().ok())
.and_then(|id| doc.get_object(id).ok())
.and_then(|o| o.as_dict().ok());
 if let Some(info) = info {
 meta.title = read_field(&doc, info, b"Title");
 meta.author = read_field(&doc, info, b"Author");
 meta.subject = read_field(&doc, info, b"Subject");
 meta.keywords = read_field(&doc, info, b"Keywords");
 meta.creator = read_field(&doc, info, b"Creator");
 meta.producer = read_field(&doc, info, b"Producer");
 }
 meta
}
