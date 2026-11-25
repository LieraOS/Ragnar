// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — page orientation detection.
//
// The skew detection was a placeholder returning Rotate0. This is
// a real implementation: tesseract's OSD mode (--psm 0) reports the
// rotation needed to upright the page; tesseract is already a
// required dependency for the OCR workflow. When it is unavailable
// or inconclusive we return R0 — a no-op, never a guess.

use super::crop::Rotation;
use super::pnm::{encode_ppm, Rgb8Image};
use std::io::Write;
use std::process::{Command, Stdio};

/// Detect the page orientation via tesseract OSD.
/// Runs `tesseract stdin stdout --psm 0` on the PPM-encoded page and
/// parses its `Rotate:` field (degrees needed to fix orientation).
pub fn detect_rotation(img: &Rgb8Image) -> Rotation {
 let ppm = encode_ppm(img);
 let mut child = match Command::new("tesseract")
.args(["stdin", "stdout", "--psm", "0"])
.stdin(Stdio::piped())
.stdout(Stdio::piped())
.stderr(Stdio::null())
.spawn()
 {
 Ok(c) => c,
 Err(_) => return Rotation::R0,
 };
 if let Some(mut stdin) = child.stdin.take() {
 // Feed the raster; a closed stdin lets tesseract proceed.
 let _ = stdin.write_all(&ppm);
 }
 let out = match child.wait_with_output() {
 Ok(o) if o.status.success() => o.stdout,
 _ => return Rotation::R0,
 };
 let text = String::from_utf8_lossy(&out);
 for line in text.lines() {
 if let Some(v) = line.strip_prefix("Rotate:") {
 return match v.trim() {
 "90" => Rotation::R90,
 "180" => Rotation::R180,
 "270" => Rotation::R270,
 _ => Rotation::R0,
 };
 }
 }
 Rotation::R0
}
