// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — redaction masking, of
// ScanPreviewWidget::redaction application: user-drawn rectangles are
// painted black *into the pixel data* — real masking, not an
// overlay — clipped to the image bounds.

use super::pnm::Rgb8Image;

/// One redaction rectangle in image pixels.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
 /// Left edge.
 pub x: i32,
 /// Top edge.
 pub y: i32,
 /// Extent right of x.
 pub width: i32,
 /// Extent below y.
 pub height: i32,
}

impl Rect {
 /// Build a rect from two corner points in any order.
 pub fn from_points(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
 Self {
 x: x1.min(x2).round() as i32,
 y: y1.min(y2).round() as i32,
 width: (x2 - x1).abs().round() as i32,
 height: (y2 - y1).abs().round() as i32,
 }
 }

 /// Clamp to image bounds; returns an empty rect when fully outside.
 pub fn clipped(&self, w: i32, h: i32) -> Self {
 let x = self.x.clamp(0, w);
 let y = self.y.clamp(0, h);
 Self {
 x,
 y,
 width: (self.x + self.width).min(w) - x,
 height: (self.y + self.height).min(h) - y,
 }
 }
}

/// Fill every rect with black, intersected with the image — the
/// exact operation the painter::fillRect(::black) performs in the
/// redaction application.
pub fn apply_redactions(img: &Rgb8Image, rects: &[Rect]) -> Rgb8Image {
 let mut out = img.clone();
 for r in rects {
 let x0 = r.x.max(0) as usize;
 let y0 = r.y.max(0) as usize;
 let x1 = ((r.x + r.width).max(0) as usize).min(img.width);
 let y1 = ((r.y + r.height).max(0) as usize).min(img.height);
 for y in y0..y1 {
 for x in x0..x1 {
 let i = (y * img.width + x) * 3;
 out.data[i] = 0;
 out.data[i + 1] = 0;
 out.data[i + 2] = 0;
 }
 }
 }
 out
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn rect_is_blacked_out() {
 let img = Rgb8Image { width: 4, height: 4, data: vec![200; 48] };
 let out = apply_redactions(&img, &[Rect { x: 1, y: 1, width: 2, height: 2 }]);
 assert_eq!(&out.data[(1 * 4 + 1) * 3..(1 * 4 + 1) * 3 + 3], &[0, 0, 0]);
 assert_eq!(&out.data[(2 * 4 + 2) * 3..(2 * 4 + 2) * 3 + 3], &[0, 0, 0]);
 assert_eq!(&out.data[0..3], &[200, 200, 200]); // outside untouched
 }

 #[test]
 fn rect_clipped_to_bounds() {
 let img = Rgb8Image { width: 2, height: 2, data: vec![200; 12] };
 let out = apply_redactions(&img, &[Rect { x: -5, y: 0, width: 10, height: 1 }]);
 assert_eq!(&out.data[0..6], &[0, 0, 0, 0, 0, 0]);
 assert_eq!(&out.data[6..9], &[200, 200, 200]);
 }
}
