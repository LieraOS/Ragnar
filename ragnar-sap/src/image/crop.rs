// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — content bounds + crop/rotate,.
//
// All operations work on packed RGB8 buffers (3 bytes per pixel),
// identical to the design.

use super::pnm::Rgb8Image;

/// Content pixel threshold — a channel below 240 marks content.
const CONTENT_THRESHOLD: u8 = 240;

/// Rotation in 90-degree steps, mirroring rotation enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
 /// No rotation.
 R0,
 /// Clockwise quarter turn.
 R90,
 /// Half turn.
 R180,
 /// Counter-clockwise quarter turn.
 R270,
}

/// Content bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropBox {
 /// Left edge.
 pub x: i32,
 /// Top edge.
 pub y: i32,
 /// Extent right of x.
 pub width: i32,
 /// Extent below y.
 pub height: i32,
}

/// Any channel below the threshold counts as content — the 
/// isPixelContent.
fn is_content(r: u8, g: u8, b: u8) -> bool {
 r < CONTENT_THRESHOLD || g < CONTENT_THRESHOLD || b < CONTENT_THRESHOLD
}

#[inline]
fn pixel_at(data: &[u8], width: usize, x: usize, y: usize) -> (u8, u8, u8) {
 let i = (y * width + x) * 3;
 (data[i], data[i + 1], data[i + 2])
}

/// First content x in a row; width when empty.
fn row_content_start(img: &Rgb8Image, y: usize) -> usize {
 for x in 0..img.width {
 let (r, g, b) = pixel_at(&img.data, img.width, x, y);
 if is_content(r, g, b) {
 return x;
 }
 }
 img.width
}

/// One past the last content x in a row; 0 when empty.
fn row_content_end(img: &Rgb8Image, y: usize) -> usize {
 for x in (0..img.width).rev() {
 let (r, g, b) = pixel_at(&img.data, img.width, x, y);
 if is_content(r, g, b) {
 return x + 1;
 }
 }
 0
}

/// Bounding box of all non-white pixels — same sweep order as the
/// (top edge, bottom edge, then per-row extents within them).
pub fn detect_content_bounds(img: &Rgb8Image) -> CropBox {
 let mut boxy = CropBox {
 x: 0,
 y: 0,
 width: img.width as i32,
 height: img.height as i32,
 };
 if img.data.is_empty() || img.width == 0 || img.height == 0 {
 return boxy;
 }
 'top: for y in 0..img.height {
 for x in 0..img.width {
 let (r, g, b) = pixel_at(&img.data, img.width, x, y);
 if is_content(r, g, b) {
 boxy.y = y as i32;
 break 'top;
 }
 }
 }
 'bottom: for y in (boxy.y as usize..img.height).rev() {
 for x in 0..img.width {
 let (r, g, b) = pixel_at(&img.data, img.width, x, y);
 if is_content(r, g, b) {
 boxy.height = y as i32 - boxy.y + 1;
 break 'bottom;
 }
 }
 }
 let (y0, y1) = (boxy.y as usize, (boxy.y + boxy.height) as usize);
 let mut min_x = img.width;
 let mut max_x = 0usize;
 for y in y0..y1 {
 min_x = min_x.min(row_content_start(img, y));
 max_x = max_x.max(row_content_end(img, y));
 }
 boxy.x = min_x as i32;
 boxy.width = max_x as i32 - boxy.x;
 boxy
}

/// Crop to the given box.
pub fn crop(img: &Rgb8Image, b: &CropBox) -> Rgb8Image {
 let (bw, bh) = (b.width.max(0) as usize, b.height.max(0) as usize);
 let mut data = vec![0u8; bw * bh * 3];
 for y in 0..bh {
 for x in 0..bw {
 let src = ((b.y as usize + y) * img.width + (b.x as usize + x)) * 3;
 let dst = (y * bw + x) * 3;
 if src + 2 < img.data.len() {
 data[dst..dst + 3].copy_from_slice(&img.data[src..src + 3]);
 }
 }
 }
 Rgb8Image {
 width: bw,
 height: bh,
 data,
 }
}

/// Crop to the detected content bounds — autoCrop.
pub fn auto_crop(img: &Rgb8Image) -> Rgb8Image {
 crop(img, &detect_content_bounds(img))
}

/// Rotate by a quarter-turn step — same index math.
pub fn rotate(img: &Rgb8Image, angle: Rotation) -> Rgb8Image {
 match angle {
 Rotation::R0 => img.clone(),
 Rotation::R180 => {
 let (w, h) = (img.width, img.height);
 let mut data = vec![0u8; img.data.len()];
 for y in 0..h {
 for x in 0..w {
 let src = (y * w + x) * 3;
 let dst = ((h - 1 - y) * w + (w - 1 - x)) * 3;
 data[dst..dst + 3].copy_from_slice(&img.data[src..src + 3]);
 }
 }
 Rgb8Image { width: w, height: h, data }
 }
 Rotation::R90 | Rotation::R270 => {
 let (w, h) = (img.width, img.height);
 let mut data = vec![0u8; img.data.len()];
 for y in 0..h {
 for x in 0..w {
 let src = (y * w + x) * 3;
 let dst = if angle == Rotation::R90 {
 (x * h + (h - 1 - y)) * 3
 } else {
 ((w - 1 - x) * h + y) * 3
 };
 data[dst..dst + 3].copy_from_slice(&img.data[src..src + 3]);
 }
 }
 Rgb8Image { width: h, height: w, data }
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 /// 4x4 white image with one black pixel at (2,1).
 fn test_img() -> Rgb8Image {
 let mut data = vec![255u8; 4 * 4 * 3];
 let i = (1 * 4 + 2) * 3;
 data[i] = 0;
 data[i + 1] = 0;
 data[i + 2] = 0;
 Rgb8Image { width: 4, height: 4, data }
 }

 #[test]
 fn bounds_find_the_pixel() {
 let b = detect_content_bounds(&test_img());
 assert_eq!((b.x, b.y, b.width, b.height), (2, 1, 1, 1));
 }

 #[test]
 fn crop_extracts_exactly() {
 let img = test_img();
 let b = CropBox { x: 1, y: 0, width: 2, height: 2 };
 let c = crop(&img, &b);
 assert_eq!((c.width, c.height), (2, 2));
 assert_eq!(&c.data[0..3], &[255, 255, 255]); // top-left of crop
 assert_eq!(&c.data[3 * 3..3 * 3 + 3], &[0, 0, 0]); // the pixel
 }

 #[test]
 fn rotate_90_swaps_axes() {
 let c = rotate(&test_img(), Rotation::R90);
 assert_eq!((c.width, c.height), (4, 4));
 // (2,1) -> dst index (x*h + h-1-y) = 2*4 + 2 = 10 -> (x',y')=(2,2)
 let i = (2 * 4 + 2) * 3;
 assert_eq!(&c.data[i..i + 3], &[0, 0, 0]);
 }

 #[test]
 fn rotate_180_mirrors() {
 let c = rotate(&test_img(), Rotation::R180);
 // (2,1) -> (4-1-2, 4-1-1) = (1,2)
 let i = (2 * 4 + 1) * 3;
 assert_eq!(&c.data[i..i + 3], &[0, 0, 0]);
 }
}
