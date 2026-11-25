// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — PNM decoding into the RGB8 buffers the pipeline
// operates on (uses the image; we keep raw u8 RGB triplets).

/// A decoded raster page: tightly packed RGB8.
#[derive(Debug, Clone)]
pub struct Rgb8Image {
 /// Pixels per row.
 pub width: usize,
 /// Rows.
 pub height: usize,
 /// `width*height*3` bytes, row-major.
 pub data: Vec<u8>,
}

/// Parse P4/P5/P6 (raw PNM) — the formats Session::perform_scan
/// emits. P4/P5 are expanded to RGB so every pipeline stage sees
/// one pixel format.
pub fn decode(pnm: &[u8]) -> Option<Rgb8Image> {
 let mut it = TokenIter::new(pnm);
 let magic = it.next()?;
 let (bpp, _gray): (usize, bool) = match magic {
 b"P6" => (3, false),
 b"P5" => (1, true),
 b"P4" => (0, true), // 1-bit packed
 _ => return None,
 };
 let width = it.next_usize()?;
 let height = it.next_usize()?;
 if bpp > 0 {
 // P5/P6 carry a maxval field before the raster.
 it.next_usize()?;
 }
 let raster = it.rest();
 if width == 0 || height == 0 {
 return None;
 }
 match bpp {
 3 => {
 let need = width * height * 3;
 if raster.len() < need {
 return None;
 }
 Some(Rgb8Image {
 width,
 height,
 data: raster[..need].to_vec(),
 })
 }
 1 => {
 let need = width * height;
 if raster.len() < need {
 return None;
 }
 let mut data = Vec::with_capacity(need * 3);
 for &v in &raster[..need] {
 data.extend_from_slice(&[v, v, v]);
 }
 Some(Rgb8Image { width, height, data })
 }
 _ => {
 // P4: MSB-first packed bits, 0 = black. actually PBM
 // encodes 1 = black; scanimage emits that convention.
 let stride = width.div_ceil(8);
 if raster.len() < stride * height {
 return None;
 }
 let mut data = Vec::with_capacity(width * height * 3);
 for row in raster[..stride * height].chunks(stride) {
 for x in 0..width {
 let byte = row[x / 8];
 let bit = (byte >> (7 - x % 8)) & 1;
 let v = if bit == 1 { 0u8 } else { 255u8 };
 data.extend_from_slice(&[v, v, v]);
 }
 }
 Some(Rgb8Image { width, height, data })
 }
 }
}

/// Encode RGB8 as P6 — the form print/PDF export consumes.
pub fn encode_ppm(img: &Rgb8Image) -> Vec<u8> {
 let mut out = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
 out.extend_from_slice(&img.data);
 out
}

/// Whitespace-tolerant token scanner over the PNM header, including
/// `# comment` handling per the PNM spec.
struct TokenIter<'a> {
 buf: &'a [u8],
 pos: usize,
}

impl<'a> TokenIter<'a> {
 fn new(buf: &'a [u8]) -> Self {
 Self { buf, pos: 0 }
 }

 fn next(&mut self) -> Option<&'a [u8]> {
 loop {
 while self.pos < self.buf.len() && self.buf[self.pos].is_ascii_whitespace() {
 self.pos += 1;
 }
 if self.pos < self.buf.len() && self.buf[self.pos] == b'#' {
 while self.pos < self.buf.len() && self.buf[self.pos] != b'\n' {
 self.pos += 1;
 }
 continue;
 }
 break;
 }
 let start = self.pos;
 while self.pos < self.buf.len() && !self.buf[self.pos].is_ascii_whitespace() {
 self.pos += 1;
 }
 if self.pos == start {
 None
 } else {
 Some(&self.buf[start..self.pos])
 }
 }

 fn next_usize(&mut self) -> Option<usize> {
 std::str::from_utf8(self.next()?).ok()?.parse().ok()
 }

 /// Remaining bytes after the last header token — the raster
 /// starts right after one whitespace separator.
 fn rest(&mut self) -> &'a [u8] {
 if self.pos < self.buf.len() && self.buf[self.pos].is_ascii_whitespace() {
 self.pos += 1;
 }
 &self.buf[self.pos..]
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn decode_p5_expands_to_rgb() {
 let pgm = b"P5\n2 2\n255\n\x00\x80\xC0\xFF";
 let img = decode(pgm).expect("p5 decodes");
 assert_eq!((img.width, img.height), (2, 2));
 assert_eq!(&img.data[0..6], &[0, 0, 0, 0x80, 0x80, 0x80]);
 }

 #[test]
 fn decode_p6_passthrough() {
 let ppm = b"P6\n1 1\n255\n\x11\x22\x33";
 let img = decode(ppm).expect("p6 decodes");
 assert_eq!(&img.data, &[0x11, 0x22, 0x33]);
 }

 #[test]
 fn decode_p4_unpacks_bits() {
 // PBM: 1 = black; byte 0xA0 = 10100000 -> black, white, black
 let pbm = b"P4\n3 1\n\xA0";
 let img = decode(pbm).expect("p4 decodes");
 assert_eq!(&img.data[0..9], &[0, 0, 0, 255, 255, 255, 0, 0, 0]);
 }

 #[test]
 fn ppm_roundtrip() {
 let img = Rgb8Image { width: 2, height: 1, data: vec![1, 2, 3, 4, 5, 6] };
 let enc = encode_ppm(&img);
 let dec = decode(&enc).expect("roundtrip");
 assert_eq!(dec.data, img.data);
 }
}
