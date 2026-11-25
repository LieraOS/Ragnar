// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — decode via the `image` crate.
//
// Covers the common raster formats in pure Rust: PNG, JPEG, GIF
// (first frame for now), WebP, TIFF, BMP, TGA, ICO/CUR, PNM, QOI,
// DDS, EXR, HDR, Farbfeld, APNG. Detection happens inside the
// crate by content — extension mislabels don't matter.

use super::Decoded;
use image::ImageReader;

/// Decode the first frame of `path` into RGBA8.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let img = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Decoded {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}
