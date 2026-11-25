// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — document → page images for the print-only preview
//
// The rendered PDFs via poppler, images via the image and text/HTML
// via the text layout. Here: pdftoppm rasterizes PDFs (poppler, dep
// already required), the image crate decodes raster files, and
// pangocairo lays out plain text / markdown / stripped HTML onto
// A4-ish pages — same preview contract, no WebEngine needed.


use crate::image::pnm::{self, Rgb8Image};

/// Preview raster resolution (dpi).
const PREVIEW_DPI: i32 = 96;

/// Render `path` into a list of page images (empty on failure).
pub fn render(path: &str) -> Vec<Rgb8Image> {
 match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
 "pdf" => render_pdf(path),
 "png" | "jpg" | "jpeg" | "bmp" | "tiff" | "tif" | "webp" => render_image(path),
 _ => render_text(path),
 }
}

/// pdftoppm → PPM pages → decode.
fn render_pdf(path: &str) -> Vec<Rgb8Image> {
 let dir = std::env::temp_dir().join(format!("ragnar-sap-pv-{}", std::process::id()));
 let _ = std::fs::create_dir_all(&dir);
 let prefix = dir.join("page");
 let ok = std::process::Command::new("pdftoppm")
.args(["-r", &PREVIEW_DPI.to_string(), path, &prefix.to_string_lossy()])
.output()
.map(|o| o.status.success())
.unwrap_or(false);
 if !ok {
 return Vec::new();
 }
 let mut pages = Vec::new();
 let mut entries: Vec<_> = std::fs::read_dir(&dir)
.map(|rd| rd.flatten().map(|e| e.path()).collect())
.unwrap_or_default();
 entries.sort();
 for p in entries {
 if p.extension().map(|e| e == "ppm").unwrap_or(false) {
 if let Some(img) = std::fs::read(&p).ok().and_then(|b| pnm::decode(&b)) {
 pages.push(img);
 }
 }
 }
 let _ = std::fs::remove_dir_all(&dir);
 pages
}

/// image crate decode → RGB8.
fn render_image(path: &str) -> Vec<Rgb8Image> {
 image::open(path)
.ok()
.map(|im| {
 let rgb = im.to_rgb8();
 Rgb8Image {
 width: rgb.width() as usize,
 height: rgb.height() as usize,
 data: rgb.into_raw(),
 }
 })
.into_iter()
.collect()
}

/// Text/markdown/HTML → pangocairo pages (~A4 @96dpi = 794×1123).
fn render_text(path: &str) -> Vec<Rgb8Image> {
 let Ok(raw) = std::fs::read_to_string(path) else { return Vec::new() };
 // HTML: drop tags so the text content is what gets printed —
 // the text layout formatted it, we render the same characters.
 let text = if path.to_lowercase().ends_with("htm") || path.to_lowercase().ends_with("html") {
 let mut t = String::with_capacity(raw.len());
 let mut in_tag = false;
 for c in raw.chars() {
 match c {
 '<' => in_tag = true,
 '>' => in_tag = false,
 _ if !in_tag => t.push(c),
 _ => {}
 }
 }
 t
 } else {
 raw
 };

 let (w, h, margin) = (794i32, 1123i32, 60i32);
 let mut pages = Vec::new();
 let mut remaining: &str = text.trim_end();
 while !remaining.is_empty() {
 let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::Rgb24, w, h)
.expect("surface");
 {
 let ctx = gtk4::cairo::Context::new(&surface).expect("ctx");
 ctx.set_source_rgb(1.0, 1.0, 1.0);
 let _ = ctx.paint();
 let layout = pangocairo::functions::create_layout(&ctx);
 layout.set_width(((w - 2 * margin) as f64 * gtk4::pango::SCALE as f64) as i32);
 layout.set_text(remaining);
 // How many lines fit on one page?
 let mut y = margin;
 let mut consumed = 0usize;
 let mut iter = layout.iter();
 while let Some(line) = iter.line_readonly() {
 let (_ink, logical) = line.extents();
 let lh = logical.height() / gtk4::pango::SCALE;
 if y + lh > h - margin {
 break;
 }
 ctx.set_source_rgb(0.0, 0.0, 0.0);
 ctx.move_to(margin as f64, y as f64);
 pangocairo::functions::show_layout_line(&ctx, &line);
 y += lh;
 consumed = (line.start_index() + line.length()) as usize;
 if !iter.next_line() {
 break;
 }
 }
 remaining = if consumed >= remaining.len() { "" } else { &remaining[consumed..] };
 }
 pages.push(surface_to_rgb(&mut surface));
 }
 if pages.is_empty() {
 let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::Rgb24, w, h).expect("s");
 pages.push(surface_to_rgb(&mut surface));
 }
 pages
}

/// cairo Rgb24 surface → Rgb8Image (BGRX → RGB).
fn surface_to_rgb(surface: &mut gtk4::cairo::ImageSurface) -> Rgb8Image {
 let (w, h) = (surface.width() as usize, surface.height() as usize);
 let stride = surface.stride() as usize;
 let data = surface.data().expect("data");
 let mut out = vec![0u8; w * h * 3];
 for y in 0..h {
 for x in 0..w {
 let si = y * stride + x * 4;
 let di = (y * w + x) * 3;
 out[di] = data[si + 2];
 out[di + 1] = data[si + 1];
 out[di + 2] = data[si];
 }
 }
 drop(data);
 Rgb8Image { width: w, height: h, data: out }
}
