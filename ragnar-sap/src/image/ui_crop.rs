// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan-window auto-crop
//
// (NOT core/auto_crop — this one estimates
// the scanner-lid background luminance from corner patches, counts
// per-row/column pixels that differ from it, applies density
// thresholds, adds a safety margin and rejects degenerate boxes).

use super::crop::CropBox;
use super::pnm::Rgb8Image;

/// Luminance deviation that counts a pixel as content.
const CONTENT_THRESHOLD: i32 = 22;
/// Fraction of a row/column that must be content for it to count.
const ROW_COL_DENSITY: f64 = 0.005;
/// Safety margin around the detected box (fraction + minimum px).
const MARGIN_FRACTION: f64 = 0.01;
const MARGIN_MIN_PX: i64 = 6;
/// Guardrails: degenerate boxes keep the original image.
const MIN_CONTENT_FRACTION: f64 = 0.05;
const MAX_CONTENT_FRACTION: f64 = 0.95;

/// Median luminance of the four corner patches — best guess for the
/// scanner-lid background colour.
fn estimate_background_luminance(img: &Rgb8Image) -> i32 {
 let pw = (img.width / 20).max(4);
 let ph = (img.height / 20).max(4);
 let mut samples = Vec::with_capacity(pw * ph * 4);
 let corners = [
 (0usize, 0usize),
 (img.width - pw, 0),
 (0, img.height - ph),
 (img.width - pw, img.height - ph),
 ];
 for &(cx, cy) in &corners {
 for y in cy..(cy + ph).min(img.height) {
 for x in cx..(cx + pw).min(img.width) {
 let i = (y * img.width + x) * 3;
 // Grayscale8-equivalent luminance (BT.601).
 let l = (img.data[i] as i32 * 299
 + img.data[i + 1] as i32 * 587
 + img.data[i + 2] as i32 * 114)
 / 1000;
 samples.push(l);
 }
 }
 }
 samples.sort_unstable();
 if samples.is_empty() { 255 } else { samples[samples.len() / 2] }
}

/// Detect the content box by background-difference density — the
/// exact algorithm scan window applied before previewing.
/// Returns `None` when no trustworthy box exists (keep original).
pub fn detect_ui_crop(img: &Rgb8Image) -> Option<CropBox> {
 if img.width < 64 || img.height < 64 {
 return None;
 }
 let bg = estimate_background_luminance(img);
 let (w, h) = (img.width, img.height);
 let row_threshold = (w as f64 * ROW_COL_DENSITY) as i64;
 let col_threshold = (h as f64 * ROW_COL_DENSITY) as i64;

 let mut row_counts = vec![0i64; h];
 let mut col_counts = vec![0i64; w];
 for (i, px) in img.data.chunks_exact(3).enumerate() {
 let l = (px[0] as i32 * 299 + px[1] as i32 * 587 + px[2] as i32 * 114) / 1000;
 if (l - bg).abs() > CONTENT_THRESHOLD {
 let (x, y) = (i % w, i / w);
 row_counts[y] += 1;
 col_counts[x] += 1;
 }
 }

 let first_above = |c: &[i64], t: i64| c.iter().position(|&v| v > t).map(|p| p as i64);
 let last_above = |c: &[i64], t: i64| c.iter().rposition(|&v| v > t).map(|p| p as i64);
 let (top, bottom) = (first_above(&row_counts, row_threshold)?,
 last_above(&row_counts, row_threshold)?);
 let (left, right) = (first_above(&col_counts, col_threshold)?,
 last_above(&col_counts, col_threshold)?);

 let mx = (w as i64 as f64 * MARGIN_FRACTION) as i64;
 let my = (h as i64 as f64 * MARGIN_FRACTION) as i64;
 let mx = mx.max(MARGIN_MIN_PX);
 let my = my.max(MARGIN_MIN_PX);
 let x = (left - mx).max(0);
 let y = (top - my).max(0);
 let bw = (right + mx).min(w as i64 - 1) - x + 1;
 let bh = (bottom + my).min(h as i64 - 1) - y + 1;

 let frac = (bw * bh) as f64 / (w * h) as f64;
 if !(MIN_CONTENT_FRACTION..=MAX_CONTENT_FRACTION).contains(&frac) {
 return None;
 }
 Some(CropBox {
 x: x as i32,
 y: y as i32,
 width: bw as i32,
 height: bh as i32,
 })
}

/// Apply the UI auto-crop; returns the original when detection fails.
pub fn auto_crop_ui(img: &Rgb8Image) -> Rgb8Image {
 match detect_ui_crop(img) {
 Some(b) => super::crop::crop(img, &b),
 None => img.clone(),
 }
}
