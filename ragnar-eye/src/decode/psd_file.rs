// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — Photoshop PSD/PSB via the `psd` crate.
//
// Returns the flattened composite image — the same surface
// surface shows (the composite image, not the
// individual layers).

use super::Decoded;

/// Decode the composite image of a .psd/.psb file.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let doc = psd::Psd::from_bytes(&bytes).map_err(|e| format!("{e:?}"))?;
    let rgba8 = doc.rgba();
    Ok(Decoded {
        width: doc.width(),
        height: doc.height(),
        rgba: rgba8,
    })
}
