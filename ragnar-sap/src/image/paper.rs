// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — paper format detection, of
// PaperFormatDetector: pixels -> mm at scan DPI, then nearest
// standard format with orientation normalisation.

/// A standard paper format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperSize {
 /// Short name ("A4", "Letter",.).
 pub name: &'static str,
 /// Width in mm.
 pub width_mm: f64,
 /// Height in mm.
 pub height_mm: f64,
}

/// The getStandardFormats table.
pub const STANDARD_FORMATS: &[PaperSize] = &[
 PaperSize { name: "A4", width_mm: 210.0, height_mm: 297.0 },
 PaperSize { name: "A5", width_mm: 148.0, height_mm: 210.0 },
 PaperSize { name: "A3", width_mm: 297.0, height_mm: 420.0 },
 PaperSize { name: "Letter", width_mm: 215.9, height_mm: 279.4 },
 PaperSize { name: "Legal", width_mm: 215.9, height_mm: 355.6 },
 PaperSize { name: "Tabloid", width_mm: 279.4, height_mm: 431.8 },
 PaperSize { name: "Ledger", width_mm: 431.8, height_mm: 279.4 },
];

/// pixels -> mm; dpi <= 0 falls back to 300 like the.
pub fn pixels_to_mm(pixels: i32, dpi: i32) -> f64 {
 let dpi = if dpi <= 0 { 300 } else { dpi };
 pixels as f64 * 25.4 / dpi as f64
}

/// Portrait when taller than wide.
pub fn is_portrait(width_px: i32, height_px: i32) -> bool {
 height_px > width_px
}

/// Nearest standard format by summed mm deltas — detectFormat.
pub fn detect_format(image_width_px: i32, image_height_px: i32, dpi: i32) -> PaperSize {
 let mut w_mm = pixels_to_mm(image_width_px, dpi);
 let mut h_mm = pixels_to_mm(image_height_px, dpi);
 let portrait = is_portrait(image_width_px, image_height_px);
 if !portrait {
 std::mem::swap(&mut w_mm, &mut h_mm);
 }
 let mut best = STANDARD_FORMATS[0];
 let mut min_diff = f64::MAX;
 for f in STANDARD_FORMATS {
 let (mut fw, mut fh) = (f.width_mm, f.height_mm);
 if fh < fw {
 std::mem::swap(&mut fw, &mut fh);
 }
 let diff = (w_mm - fw).abs() + (h_mm - fh).abs();
 if diff < min_diff {
 min_diff = diff;
 best = *f;
 if !portrait {
 std::mem::swap(&mut best.width_mm, &mut best.height_mm);
 }
 }
 }
 best
}
