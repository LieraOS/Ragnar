// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — real PDF creation from scan pages.
//
// The createPdfFromImages was a placeholder. This is a real
// writer built on lopdf: every RGB8 page becomes a /XObject image
// (DeviceRGB, FlateDecode) drawn by a /Contents stream on a page
// sized from pixels at the scan DPI — the equivalent of what
// the PDF writer produces in the version.

use crate::image::pnm::Rgb8Image;
use lopdf::{dictionary, Document, Object, Stream};

/// Wrap one Rgb8Image in a PDF image XObject stream (FlateDecode).
fn image_stream(doc: &mut Document, img: &Rgb8Image) -> lopdf::ObjectId {
 let dict = dictionary! {
 "Type" => "XObject",
 "Subtype" => "Image",
 "Width" => img.width as i64,
 "Height" => img.height as i64,
 "ColorSpace" => "DeviceRGB",
 "BitsPerComponent" => 8,
 };
 let mut stream = Stream::new(dict, img.data.clone());
 // FlateDecode keeps multi-page scans small; falls back to a raw
 // stream when compression is unavailable.
 if stream.compress().is_err() {
 stream.dict.remove(b"Filter");
 }
 doc.add_object(Object::Stream(stream))
}

/// Content stream drawing the image scaled to the full page box.
fn content_stream(doc: &mut Document, name: &str, w: f64, h: f64) -> lopdf::ObjectId {
 let ops = format!("q\n{} 0 0 {} 0 0 cm\n/{} Do\nQ\n", w, h, name);
 doc.add_object(Object::Stream(Stream::new(dictionary! {}, ops.into_bytes())))
}

/// Build a multi-page PDF from scan pages.
/// `dpi` maps pixels to points (pt = px * 72/dpi); sizes
/// pages from the scanned raster the same way.
pub fn create_pdf_from_images(images: &[Rgb8Image],
 output_path: &str,
 metadata: &super::manager::PdfMetadata,
 dpi: i32,
) -> bool {
 if images.is_empty() || !super::manager::validate_pdf_path(output_path) {
 return false;
 }
 let dpi = if dpi <= 0 { 300 } else { dpi } as f64;
 let mut doc = Document::with_version("1.5");
 let pages_id = doc.new_object_id();
 let mut kids: Vec<Object> = Vec::with_capacity(images.len());
 for (i, img) in images.iter().enumerate() {
 let name = format!("Im{}", i);
 let img_id = image_stream(&mut doc, img);
 let w_pt = img.width as f64 * 72.0 / dpi;
 let h_pt = img.height as f64 * 72.0 / dpi;
 let content_id = content_stream(&mut doc, &name, w_pt, h_pt);
 let page = dictionary! {
 "Type" => "Page",
 "Parent" => pages_id,
 "MediaBox" => vec![0.into(), 0.into(), w_pt.into(), h_pt.into()],
 "Resources" => dictionary! {
 "XObject" => dictionary! { name.clone() => img_id },
 },
 "Contents" => content_id,
 };
 kids.push(doc.add_object(Object::Dictionary(page)).into());
 }
 doc.objects.insert(pages_id,
 Object::Dictionary(dictionary! {
 "Type" => "Pages",
 "Kids" => kids.clone(),
 "Count" => kids.len() as i64,
 }),
 );
 let catalog_id = doc.add_object(Object::Dictionary(dictionary! {
 "Type" => "Catalog",
 "Pages" => pages_id,
 }));
 doc.trailer.set("Root", catalog_id);
 super::metadata::write(&mut doc, metadata);
 doc.save(output_path).is_ok()
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::pdf::manager::PdfMetadata;

 #[test]
 fn creates_valid_pdf_with_metadata() {
 let out = std::env::temp_dir().join(format!("ragnar-sap-pdf-{}.pdf", std::process::id()));
 let out = out.to_string_lossy().into_owned();
 let img = Rgb8Image {
 width: 4,
 height: 4,
 data: vec![128u8; 48],
 };
 let meta = PdfMetadata {
 title: "Test Scan".into(),
 author: "ragnar-sap".into(),
..Default::default()
 };
 assert!(create_pdf_from_images(&[img], &out, &meta, 300));
 assert!(super::super::manager::is_pdf_valid(&out));
 let read = super::super::metadata::get_from_file(&out);
 assert_eq!(read.title, "Test Scan");
 assert_eq!(read.author, "ragnar-sap");
 let _ = std::fs::remove_file(&out);
 }
}
