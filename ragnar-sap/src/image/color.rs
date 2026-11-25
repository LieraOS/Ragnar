// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — brightness/contrast/saturation, of
// ColorOptimizer plus the UI's brightness/contrast adjust.

use super::pnm::Rgb8Image;

/// Parameters chosen by analyse — same fields 
/// OptimizationParams.
#[derive(Debug, Clone, Copy)]
pub struct OptimizeParams {
 /// Additive brightness shift (-255.255); 0 = unchanged.
 pub brightness: f64,
 /// Multiplicative contrast around mid-gray; 1.0 = unchanged.
 pub contrast: f64,
 /// Multiplicative saturation; 1.0 = unchanged.
 pub saturation: f64,
}

impl Default for OptimizeParams {
 fn default() -> Self {
 Self {
 brightness: 0.0,
 contrast: 1.0,
 saturation: 1.0,
 }
 }
}

fn clamp8(v: f64) -> u8 {
 v.clamp(0.0, 255.0) as u8
}

/// Weighted luma (0.299/0.587/0.114) normalised to 0.1.
fn luma(data: &[u8], i: usize) -> f64 {
 (0.299 * data[i] as f64 + 0.587 * data[i + 1] as f64 + 0.114 * data[i + 2] as f64) / 255.0
}

/// Mean luma of the whole image.
pub fn average_brightness(img: &Rgb8Image) -> f64 {
 if img.data.is_empty() {
 return 0.0;
 }
 let mut total = 0.0;
 let mut count = 0usize;
 for i in (0..img.data.len() - 2).step_by(3) {
 total += luma(&img.data, i);
 count += 1;
 }
 if count == 0 {
 0.0
 } else {
 total / count as f64
 }
}

/// Luma standard deviation — contrast measure.
pub fn contrast(img: &Rgb8Image) -> f64 {
 if img.data.len() < 3 {
 return 0.0;
 }
 let mut lumas = Vec::with_capacity(img.data.len() / 3);
 for i in (0..img.data.len() - 2).step_by(3) {
 lumas.push(luma(&img.data, i));
 }
 let mean: f64 = lumas.iter().sum::<f64>() / lumas.len() as f64;
 let var: f64 = lumas.iter().map(|l| (l - mean) * (l - mean)).sum::<f64>() / lumas.len() as f64;
 var.sqrt()
}

/// Auto-pick correction parameters — analysis
/// thresholds verbatim.
pub fn analyse(img: &Rgb8Image) -> OptimizeParams {
 let mut p = OptimizeParams::default();
 let bright = average_brightness(img);
 let con = contrast(img);
 if bright < 0.4 {
 p.brightness = 20.0;
 } else if bright > 0.7 {
 p.brightness = -10.0;
 }
 if con < 0.15 {
 p.contrast = 1.5;
 } else if con > 0.4 {
 p.contrast = 0.8;
 }
 p
}

/// (v - 128) * factor + 128 per channel.
pub fn adjust_contrast(img: &Rgb8Image, factor: f64) -> Rgb8Image {
 let mut data = img.data.clone();
 for px in data.chunks_exact_mut(3) {
 for c in px.iter_mut() {
 *c = clamp8((*c as f64 - 128.0) * factor + 128.0);
 }
 }
 Rgb8Image { width: img.width, height: img.height, data }
}

/// Additive channel shift.
pub fn adjust_brightness(img: &Rgb8Image, shift: i32) -> Rgb8Image {
 let mut data = img.data.clone();
 for c in data.iter_mut() {
 *c = clamp8(*c as f64 + shift as f64);
 }
 Rgb8Image { width: img.width, height: img.height, data }
}

/// Combined UI slider transform from image :
/// (v - 128) * (contrastPct/100) + 128 + brightness.
/// Bound to the Scan→Print sliders in the UI phase.
#[allow(dead_code)]
pub fn adjust_brightness_contrast(img: &Rgb8Image, brightness: i32, contrast_pct: i32) -> Rgb8Image {
 if brightness == 0 && contrast_pct == 100 {
 return img.clone();
 }
 let f = contrast_pct as f64 / 100.0;
 let mut data = img.data.clone();
 for c in data.iter_mut() {
 *c = clamp8((*c as f64 - 128.0) * f + 128.0 + brightness as f64);
 }
 Rgb8Image { width: img.width, height: img.height, data }
}

/// RGB -> HSV, identical math to the.
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
 let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
 let maxc = rf.max(gf).max(bf);
 let minc = rf.min(gf).min(bf);
 let delta = maxc - minc;
 let v = maxc;
 let s = if maxc == 0.0 { 0.0 } else { delta / maxc };
 let h = if delta == 0.0 {
 0.0
 } else if maxc == rf {
 ((gf - bf) / delta) % 6.0 * 60.0
 } else if maxc == gf {
 ((bf - rf) / delta + 2.0) * 60.0
 } else {
 ((rf - gf) / delta + 4.0) * 60.0
 };
 let h = if h < 0.0 { h + 360.0 } else { h % 360.0 };
 (h, s, v)
}

/// HSV -> RGB, identical math to the.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
 let c = v * s;
 let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
 let m = v - c;
 let (rf, gf, bf) = if h < 60.0 {
 (c, x, 0.0)
 } else if h < 120.0 {
 (x, c, 0.0)
 } else if h < 180.0 {
 (0.0, c, x)
 } else if h < 240.0 {
 (0.0, x, c)
 } else if h < 300.0 {
 (x, 0.0, c)
 } else {
 (c, 0.0, x)
 };
 (clamp8((rf + m) * 255.0), clamp8((gf + m) * 255.0), clamp8((bf + m) * 255.0))
}

/// Scale saturation through the HSV round-trip.
pub fn adjust_saturation(img: &Rgb8Image, factor: f64) -> Rgb8Image {
 let mut data = img.data.clone();
 for px in data.chunks_exact_mut(3) {
 let (h, s, v) = rgb_to_hsv(px[0], px[1], px[2]);
 let (r, g, b) = hsv_to_rgb(h, (s * factor).min(1.0), v);
 px[0] = r;
 px[1] = g;
 px[2] = b;
 }
 Rgb8Image { width: img.width, height: img.height, data }
}

/// Apply a parameter set in order: contrast, brightness,
/// saturation.
pub fn apply(img: &Rgb8Image, p: &OptimizeParams) -> Rgb8Image {
 let mut out = img.clone();
 if p.contrast != 1.0 {
 out = adjust_contrast(&out, p.contrast);
 }
 if p.brightness != 0.0 {
 out = adjust_brightness(&out, p.brightness as i32);
 }
 if p.saturation != 1.0 {
 out = adjust_saturation(&out, p.saturation);
 }
 out
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn hsv_roundtrip() {
 for (r, g, b) in [(255u8, 0, 0), (0, 255, 0), (0, 0, 255), (128, 64, 200), (255, 255, 255)] {
 let (h, s, v) = rgb_to_hsv(r, g, b);
 let (r2, g2, b2) = hsv_to_rgb(h, s, v);
 assert!((r as i16 - r2 as i16).abs() <= 2, "r {r} vs {r2}");
 assert!((g as i16 - g2 as i16).abs() <= 2, "g {g} vs {g2}");
 assert!((b as i16 - b2 as i16).abs() <= 2, "b {b} vs {b2}");
 }
 }

 #[test]
 fn brightness_shifts_all_channels() {
 let img = Rgb8Image { width: 1, height: 1, data: vec![100, 110, 120] };
 let out = adjust_brightness(&img, 20);
 assert_eq!(out.data, vec![120, 130, 140]);
 let out = adjust_brightness(&img, -200);
 assert_eq!(out.data, vec![0, 0, 0]);
 }

 #[test]
 fn contrast_pivots_at_128() {
 let img = Rgb8Image { width: 1, height: 1, data: vec![128, 178, 28] };
 let out = adjust_contrast(&img, 2.0);
 // (v-128)*2+128: 128->128, 178->228, 28->28. wait (28-128)*2+128 = -72 -> clamp 0
 assert_eq!(out.data, vec![128, 228, 0]);
 }

 #[test]
 fn analyse_dark_lowcontrast() {
 let img = Rgb8Image { width: 10, height: 10, data: vec![60; 300] };
 let p = analyse(&img);
 assert_eq!(p.brightness, 20.0);
 assert_eq!(p.contrast, 1.5);
 }
}
