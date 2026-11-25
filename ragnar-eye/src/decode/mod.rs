// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — decode pipeline.
//
// decode() is the single entry point: every handler returns an
// RGBA8 buffer plus dimensions. Dispatch prefers magic bytes over
// the file extension — misnamed files still open, as
// viewers expect.

pub mod djvu;
pub mod eps;
pub mod exotics;
pub mod fits;
pub mod heif;
pub mod jxl;
pub mod psd_file;
pub mod raster;
pub mod raw;
pub mod zipbased;

/// One decoded image, 8-bit RGBA.
#[derive(Clone)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Decoded {
    /// Helper for decoders that produce RGB (pads alpha to 255).
    pub fn from_rgb(width: u32, height: u32, rgb: &[u8]) -> Decoded {
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for px in rgb.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
        Decoded {
            width,
            height,
            rgba,
        }
    }

    /// Helper for float pipelines (FITS, HDR-ish): scale each
    /// channel into u8 with a min/max stretch.
    pub fn from_f32(width: u32, height: u32, f: &[f32], channels: usize) -> Decoded {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for v in f.iter().filter(|v| v.is_finite()) {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        let range = (hi - lo).max(f32::EPSILON);
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for px in f.chunks(channels) {
            for c in 0..3 {
                let v = px.get(c).or(px.first()).copied().unwrap_or(0.0);
                rgba.push((((v - lo) / range) * 255.0).clamp(0.0, 255.0) as u8);
            }
            rgba.push(255);
        }
        Decoded {
            width,
            height,
            rgba,
        }
    }
}

/// Lowercase extension of the path, if any.
pub fn ext_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// First N bytes of the file for magic detection (64 covers every
/// signature we check, including far-out offsets like VICAR/MSP).
pub fn sniff(path: &str, n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut buf = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open(path) {
        let _ = f.read(&mut buf);
    }
    buf
}

fn has(head: &[u8], sig: &[u8]) -> bool {
    head.len() >= sig.len() && &head[..sig.len()] == sig
}

/// Decode `path` into RGBA8. Tries handlers in specificity order —
/// magic-first handlers before extension-only ones.
pub fn decode(path: &str) -> Result<Decoded, String> {
    let head = sniff(path, 64);
    let ext = ext_of(path);

    // ZIP containers first — KRA/ORA are ZIPs, PSD is not, and the
    // `image` crate would mis-detect the PK signature otherwise.
    if has(&head, b"PK\x03\x04") {
        if let Ok(d) = zipbased::decode(path) {
            return Ok(d);
        }
    }

    // Magic-signature dispatch.
    let d = if has(&head, b"II*\0") || has(&head, b"MM\0*") {
        raw::try_decode(path).or_else(|_| raster::decode(path)) // TIFF vs RAW
    } else if has(&head, b"8BPS") {
        psd_file::decode(path)
    } else if has(&head, b"ftyp") || head[4..8] == *b"ftyp" {
        heif::try_decode(path) // heic/heif/avif boxes
    } else if has(&head, b"\x0dJXL") || has(&head, &[0xff, 0x0a]) {
        jxl::try_decode(path)
    } else if has(&head, b"AT&TFORM") {
        djvu::try_decode(path)
    } else if head.starts_with(b"SIMPLE") {
        fits::try_decode(path)
    } else if head.starts_with(b"%!PS") || head.starts_with(b"\xc5\xd0\xd3\xc6") {
        eps::decode(path)
    } else {
        Err(String::new())
    };
    if let Ok(d) = d {
        return Ok(d);
    }

    // Exotic hand-rolled formats.
    if let Ok(d) = exotics::decode(path, &head, &ext) {
        return Ok(d);
    }

    // Extension-driven fallbacks for containers without unique
    // magic or whose sniff failed.
    match ext.as_str() {
        "psd" | "psb" => return psd_file::decode(path),
        "kra" | "ora" => return zipbased::decode(path),
        "cr2" | "cr3" | "crw" | "nef" | "nrw" | "arw" | "orf" | "raf" | "dng" | "rw2" | "pef"
        | "srw" | "x3f" | "erf" | "kdc" | "mrw" | "mos" | "raw" | "rwl" => {
            return raw::try_decode(path)
        }
        "heic" | "heif" | "avif" => return heif::try_decode(path),
        "jxl" => return jxl::try_decode(path),
        "djvu" | "djv" => return djvu::try_decode(path),
        "fits" | "fit" | "fts" => return fits::try_decode(path),
        "eps" | "ps" | "ai" => return eps::decode(path),
        _ => {}
    }

    // Last resort: the `image` crate guesses by content itself.
    raster::decode(path)
}

/// Extensions the viewer offers in dialogs/filters — union of all
/// handlers (used by the browser to decide what counts as "image").
pub fn supported_extensions() -> &'static [&'static str] {
    &[
        // raster via `image`
        "png", "jpg", "jpeg", "jpe", "gif", "webp", "tif", "tiff", "bmp", "tga", "icb", "vda",
        "vst", "ico", "cur", "pbm", "pgm", "ppm", "pam", "qoi", "dds", "exr", "hdr", "ff",
        "farbfeld", "apng", // wrapped containers
        "psd", "psb", "kra", "ora", // camera RAW
        "cr2", "cr3", "crw", "nef", "nrw", "arw", "orf", "raf", "dng", "rw2", "pef", "srw", "x3f",
        "erf", "kdc", "mrw", "mos", "raw", "rwl", // modern codecs
        "heic", "heif", "avif", "jxl", // documents/scientific
        "djvu", "djv", "fits", "fit", "fts", "eps", "ps", "ai",
        // ported exotics (only extensions with a working decoder —
        // TODO tracks the remaining ports)
        "phm", "pfm", "mtv", "ras", "sun", "xbm", "xpm",
    ]
}
