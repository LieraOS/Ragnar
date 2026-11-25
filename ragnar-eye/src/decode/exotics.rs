// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — hand-rolled decoders for exotic formats.
// Small formats nobody else ships: float maps, Sun RAS, MTV,
// XBM, XPM. (MSP, VICAR, VIFF, ILBM, MIFF, MNG, XWD, WBMP, PCX
// land in the next phase — see TODO.)

use super::Decoded;

/// Try every exotic decoder whose magic/extension matches.
pub fn decode(path: &str, head: &[u8], ext: &str) -> Result<Decoded, String> {
    let data = || std::fs::read(path).map_err(|e| e.to_string());
    match ext {
        "pfm" | "phm" => float_map(&data()?, head),
        "ras" | "sun" => ras(&data()?),
        "mtv" => mtv(&data()?),
        "xbm" => xbm(&String::from_utf8_lossy(&data()?)),
        "xpm" => xpm(&String::from_utf8_lossy(&data()?)),
        _ => Err("not an exotic format we handle".into()),
    }
}

/// PFM/PHM: "Pf"/"PF" magic, whitespace header w h scale, then
/// native f32 pixels — 1 channel grey or 3 channel RGB.
fn float_map(data: &[u8], head: &[u8]) -> Result<Decoded, String> {
    let channels = match head {
        h if h.starts_with(b"Pf") => 1,
        h if h.starts_with(b"PF") => 3,
        _ => return Err("not a PFM family file".into()),
    };
    // Parse "WxH scale" after the magic line.
    let text_start = 2;
    let mut parts = Vec::new();
    let mut i = text_start;
    for _ in 0..3 {
        while i < data.len() && data[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while i < data.len() && !data[i].is_ascii_whitespace() {
            i += 1;
        }
        parts.push(&data[s..i]);
    }
    i += 1; // single whitespace byte after scale
    let w: u32 = String::from_utf8_lossy(parts[0])
        .parse()
        .map_err(|_| "bad header")?;
    let h: u32 = String::from_utf8_lossy(parts[1])
        .parse()
        .map_err(|_| "bad header")?;
    if w == 0 || h == 0 || w > 65535 || h > 65535 {
        return Err("bad dimensions".into());
    }
    let n = (w * h) as usize * channels;
    if data.len() < i + n * 4 {
        return Err("truncated".into());
    }
    let mut px = Vec::with_capacity(n);
    for c in data[i..i + n * 4].chunks_exact(4) {
        px.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    }
    Ok(Decoded::from_f32(w, h, &px, channels))
}

/// Sun RAS: BE header, depth 1/8/24/32, raw or RLE-compressed.
fn ras(data: &[u8]) -> Result<Decoded, String> {
    if data.len() < 32 {
        return Err("short".into());
    }
    let be32 = |o: usize| u32::from_be_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    if be32(0) != 0x59a66a95 {
        return Err("bad magic".into());
    }
    let (w, h, depth, kind) = (be32(4), be32(8), be32(12), be32(16));
    if w == 0 || h == 0 {
        return Err("bad dims".into());
    }
    // RLE: 0x80 <count> <value> runs — classic sunraster format.
    let owned;
    let body: &[u8] = if kind == 2 {
        let mut o = Vec::new();
        let mut i = 32;
        while i < data.len() {
            if data[i] == 0x80 && i + 2 < data.len() && data[i + 1] != 0 {
                o.resize(o.len() + data[i + 1] as usize + 1, data[i + 2]);
                i += 3;
            } else {
                o.push(data[i]);
                i += 1;
            }
        }
        owned = o;
        &owned
    } else {
        &data[32..]
    };
    let n = (w * h) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    match depth {
        24 | 32 => {
            for px in body[..n.min(body.len() / (depth as usize / 8)) * (depth as usize / 8)]
                .chunks_exact(depth as usize / 8)
            {
                // RAS stores BGR(A).
                rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
        }
        8 => {
            // greyscale approximation (palette parsing skipped)
            for v in &body[..n.min(body.len())] {
                rgba.extend_from_slice(&[*v, *v, *v, 255]);
            }
        }
        _ => return Err(format!("depth {depth} unsupported")),
    }
    Ok(Decoded {
        width: w,
        height: h,
        rgba,
    })
}

/// MTV raytracer: raw header, width/height BE16, then RGB24.
fn mtv(data: &[u8]) -> Result<Decoded, String> {
    if data.len() < 4 {
        return Err("short".into());
    }
    let w = u16::from_be_bytes([data[0], data[1]]) as u32;
    let h = u16::from_be_bytes([data[2], data[3]]) as u32;
    let n = (w * h) as usize;
    if w == 0 || h == 0 || data.len() < 4 + n * 3 {
        return Err("bad mtv".into());
    }
    Ok(Decoded::from_rgb(w, h, &data[4..4 + n * 3]))
}

/// XBM: text defines width/height + hex bytes, 1bpp.
fn xbm(text: &str) -> Result<Decoded, String> {
    let w: u32 = text
        .lines()
        .find(|l| l.contains("_width"))
        .and_then(|l| l.split_whitespace().last()?.parse().ok())
        .unwrap_or(0);
    let h: u32 = text
        .lines()
        .find(|l| l.contains("_height"))
        .and_then(|l| l.split_whitespace().last()?.parse().ok())
        .unwrap_or(0);
    if w == 0 || h == 0 {
        return Err("no xbm dims".into());
    }
    let hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks(2)
        .filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
        .collect();
    let row_bytes = w.div_ceil(8) as usize;
    let mut rgba = Vec::with_capacity((w * h) as usize * 4);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let bit = bytes
                .get(y * row_bytes + x / 8)
                .map(|b| (b >> (x % 8)) & 1)
                .unwrap_or(0);
            let v = if bit == 1 { 0 } else { 255 };
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
    }
    Ok(Decoded {
        width: w,
        height: h,
        rgba,
    })
}

/// XPM: text format — "w h colors cpp" header, palette, pixels.
fn xpm(text: &str) -> Result<Decoded, String> {
    let mut lines = text.lines().filter_map(|l| {
        l.trim().strip_prefix('"')?.strip_suffix('"').or_else(|| {
            l.trim()
                .strip_prefix('"')
                .map(|s| s.trim_end_matches(',').trim_end_matches('"'))
        })
    });
    let head = lines.next().ok_or("no xpm head")?;
    let nums: Vec<usize> = head
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    if nums.len() < 4 {
        return Err("bad xpm head".into());
    }
    let (w, h, colors, cpp) = (nums[0], nums[1], nums[2], nums[3]);
    let mut palette = std::collections::HashMap::new();
    for _ in 0..colors {
        let line = lines.next().ok_or("palette cut")?;
        let key = &line[..cpp];
        let val = if line.contains("#") {
            let hex = line.split('#').nth(1).unwrap_or("000000");
            u32::from_str_radix(&hex[..6.min(hex.len())], 16).unwrap_or(0)
        } else if line.contains("None") {
            0
        } else {
            0
        };
        let a = if line.contains("None") { 0 } else { 255 };
        palette.insert(key.to_string(), (val, a));
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for _ in 0..h {
        let line = lines.next().ok_or("pixels cut")?;
        for x in 0..w {
            let key = &line[x * cpp..(x * cpp + cpp).min(line.len())];
            let (v, a) = palette.get(key).copied().unwrap_or((0, 0));
            rgba.extend_from_slice(&[
                ((v >> 16) & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
                a,
            ]);
        }
    }
    Ok(Decoded {
        width: w as u32,
        height: h as u32,
        rgba,
    })
}
