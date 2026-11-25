// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — ZIP-container formats: Krita (.kra) and OpenRaster
// (.ora). Both carry a merged/preview PNG inside the archive —
// decode that, matching what a viewer typically displays.

use super::Decoded;
use std::io::Read;

/// Candidate member names inside the archive, in preference order.
const MERGED_NAMES: &[&str] = &["mergedimage.png", "merged_image.png", "preview.png"];

/// Open the archive, find the merged raster, decode it.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for name in MERGED_NAMES {
        if let Ok(mut entry) = zip.by_name(name) {
            let mut png = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut png).map_err(|e| e.to_string())?;
            let img = image::load_from_memory(&png).map_err(|e| e.to_string())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            return Ok(Decoded {
                width,
                height,
                rgba: rgba.into_raw(),
            });
        }
    }
    Err(format!("no merged image member in {path}"))
}
