// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — PDF management,.
//
// Same external tools as (ghostscript, pdftk, pdftoppm,
// tesseract, pdfinfo) but argv-based. createPdfFromImages and the
// metadata pair were placeholders in — here they are real
// implementations built on lopdf (see create.rs / metadata.rs).

use super::tools;

/// PDF metadata fields — PdfMetadata struct.
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
 /// Document title.
 pub title: String,
 /// Author.
 pub author: String,
 /// Subject.
 pub subject: String,
 /// Keywords.
 pub keywords: String,
 /// Creator application.
 pub creator: String,
 /// Producer.
 pub producer: String,
}

/// Compression settings — CompressionSettings.
#[derive(Debug, Clone, Copy)]
pub struct CompressionSettings {
 /// Target image resolution in dpi (-r flag).
 pub image_resolution: i32,
}

///.pdf suffix check — validatePdfPath.
pub fn validate_pdf_path(path: &str) -> bool {
 path.len() >= 4 && path[path.len() - 4..] == *".pdf"
}

/// Combine several PDFs via ghostscript — combinePdfs.
pub fn combine_pdfs(inputs: &[String], output: &str, _metadata: &PdfMetadata) -> bool {
 if inputs.is_empty() || !validate_pdf_path(output) {
 return false;
 }
 let out_arg = format!("-sOutputFile={}", output);
 let mut argv = vec![
 "gs", "-q", "-dNOPAUSE", "-dBATCH", "-sDEVICE=pdfwrite", out_arg.as_str(),
 ];
 argv.extend(inputs.iter().map(|s| s.as_str()));
 tools::run(&argv)
}

/// Delete one page via pdftk — deletePage.
pub fn delete_page(input: &str, output: &str, page: i32) -> bool {
 if !validate_pdf_path(input) || !validate_pdf_path(output) {
 return false;
 }
 let before = format!("1-{}", page - 1);
 let after = format!("{}-end", page + 1);
 tools::run(&["pdftk", input, "cat", &before, &after, "output", output])
}

/// Reorder pages via pdftk — reorderPages.
pub fn reorder_pages(input: &str, output: &str, order: &[i32]) -> bool {
 if !validate_pdf_path(input) || !validate_pdf_path(output) || order.is_empty() {
 return false;
 }
 let list: Vec<String> = order.iter().map(|n| n.to_string()).collect();
 let mut argv = vec!["pdftk", input, "cat"];
 argv.extend(list.iter().map(|s| s.as_str()));
 argv.extend(["output", output]);
 tools::run(&argv)
}

/// Rotate one page via pdftk — rotatePage.
/// Degrees map to pdftk compass codes: 90→R, 180→D, 270→L.
pub fn rotate_page(input: &str, output: &str, page: i32, degrees: i32) -> bool {
 if !validate_pdf_path(input) || !validate_pdf_path(output) {
 return false;
 }
 if degrees % 90 != 0 {
 return false;
 }
 let rot = match degrees.rem_euclid(360) {
 90 => "R",
 180 => "D",
 270 => "L",
 _ => "N",
 };
 let before = format!("1-{}", page - 1);
 let rotated = format!("{}{}", page, rot);
 let after = format!("{}-end", page + 1);
 tools::run(&["pdftk", input, "cat", &before, &rotated, &after, "output", output])
}

/// Compress via ghostscript — compressPdf with the same flags.
pub fn compress_pdf(input: &str, output: &str, settings: &CompressionSettings) -> bool {
 if !validate_pdf_path(input) || !validate_pdf_path(output) {
 return false;
 }
 let res = format!("-r{}", settings.image_resolution);
 let out = format!("-sOutputFile={}", output);
 tools::run(&[
 "gs", "-sDEVICE=pdfwrite", "-dCompatibilityLevel=1..4",
 "-dPDFSETTINGS=/ebook", "-dNOPAUSE", "-dQUIET", "-dBATCH",
 "-dDetectDuplicateImages", &res, &out, input,
 ])
}

/// Export pages to raster via pdftoppm — exportToTiff.
pub fn export_to_tiff(input: &str, output: &str, start_page: i32, end_page: i32) -> bool {
 if !validate_pdf_path(input) {
 return false;
 }
 let mut argv = vec!["pdftoppm"];
 let first = format!("{}", start_page);
 let last = format!("{}", end_page);
 if start_page >= 0 && end_page >= start_page {
 argv.push("-f");
 argv.push(&first);
 argv.push("-l");
 argv.push(&last);
 }
 argv.push(input);
 argv.push(output);
 tools::run(&argv)
}

/// OCR a PDF via tesseract — performOcr.
pub fn perform_ocr(input: &str, output: &str, language: &str) -> bool {
 if !validate_pdf_path(input) {
 return false;
 }
 tools::run(&["tesseract", input, output, "-l", language, "pdf"])
}

/// Page count via pdfinfo — getPageCount, parsing the Pages:
/// field directly instead of piping through grep/awk.
pub fn page_count(path: &str) -> i32 {
 if !validate_pdf_path(path) {
 return 0;
 }
 tools::capture(&["pdfinfo", path])
.and_then(|out| {
 out.lines()
.find_map(|l| l.strip_prefix("Pages:"))
.and_then(|v| v.trim().parse().ok())
 })
.unwrap_or(0)
}

/// %PDF magic check — isPdfValid.
pub fn is_pdf_valid(path: &str) -> bool {
 if !validate_pdf_path(path) {
 return false;
 }
 std::fs::read(path)
.map(|b| b.len() >= 4 && &b[..4] == b"%PDF")
.unwrap_or(false)
}
