// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — blank page detection,.

use super::pnm::Rgb8Image;

/// 95 % white pixels mark a page as empty (kDefaultEmptyThreshold).
pub const DEFAULT_EMPTY_THRESHOLD: f64 = 0.95;

/// White test — all channels at/above 240, same as default.
fn is_white(r: u8, g: u8, b: u8) -> bool {
 r >= 240 && g >= 240 && b >= 240
}

/// Fraction of white pixels (0.0–1.0).
pub fn white_pixel_ratio(img: &Rgb8Image) -> f64 {
 if img.data.is_empty() || img.width == 0 || img.height == 0 {
 return 1.0;
 }
 let mut white = 0usize;
 for px in img.data.chunks_exact(3) {
 if is_white(px[0], px[1], px[2]) {
 white += 1;
 }
 }
 white as f64 / (img.width * img.height) as f64
}

/// True when the page meets the empty threshold.
pub fn is_page_empty(img: &Rgb8Image, threshold: f64) -> bool {
 white_pixel_ratio(img) >= threshold
}

/// Count non-white pixels — content pixel count.
/// Used by the page list stats in the UI phase.
#[allow(dead_code)]
pub fn content_pixel_count(img: &Rgb8Image) -> usize {
 img.data
.chunks_exact(3)
.filter(|px| !is_white(px[0], px[1], px[2]))
.count()
}

/// Indices of non-empty pages — the empty-page filter.
/// Used by the multi-page workflow option.
#[allow(dead_code)]
pub fn filter_empty_pages(pages: &[Rgb8Image], threshold: f64) -> Vec<usize> {
 pages
.iter()
.enumerate()
.filter(|(_, p)| !is_page_empty(p, threshold))
.map(|(i, _)| i)
.collect()
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn all_white_is_empty() {
 let img = Rgb8Image { width: 4, height: 4, data: vec![255; 48] };
 assert!(is_page_empty(&img, DEFAULT_EMPTY_THRESHOLD));
 assert_eq!(content_pixel_count(&img), 0);
 }

 #[test]
 fn one_dark_pixel_means_content() {
 let mut data = vec![255u8; 48];
 data[0] = 0;
 let img = Rgb8Image { width: 4, height: 4, data };
 // 1/16 = 6.25% content -> white ratio 93.75% < 95%
 assert!(!is_page_empty(&img, DEFAULT_EMPTY_THRESHOLD));
 assert_eq!(filter_empty_pages(&[img], DEFAULT_EMPTY_THRESHOLD), vec![0]);
 }
}
