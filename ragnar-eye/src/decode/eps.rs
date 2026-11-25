// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — EPS/PS/AI via ghostscript CLI.
//
// Rasterize through `gs` into a temp
// PNG, then decode that. All argv-based, no shell interpolation
// (their audit fixed exactly that class of bug).

use super::Decoded;
use std::io::Write;

/// Rasterize a PostScript-family file via ghostscript at 150 dpi.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let tmp = std::env::temp_dir().join(format!(
        "ragnar-eye-eps-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let status = std::process::Command::new("gs")
        .args([
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            "-dQUIET",
            "-sDEVICE=pngalpha",
            "-r150",
        ])
        .arg(format!("-sOutputFile={}", tmp.display()))
        .arg(path)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() || !tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
        return Err("ghostscript rasterize failed".into());
    }
    let out = super::raster::decode(tmp.to_string_lossy().as_ref());
    let _ = std::fs::remove_file(&tmp);
    let _ = std::io::stderr().flush();
    out
}
