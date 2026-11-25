// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — lightweight in-place filters, the Eye-of-GNOME
// style "a couple of useful ones" the user asked for.
// All operate on the decoded RGBA buffer in-place.

use crate::decode::Decoded;

/// Available filters, in menu order.
pub const FILTERS: &[(&str, &str)] = &[
    ("greyscale", "Graustufen"),
    ("sepia", "Sepia"),
    ("invert", "Invertieren"),
    ("enhance", "Automatisch verbessern"),
    ("redeye", "Rote-Augen-Reduktion"),
];

/// Apply a named filter to the decoded image.
pub fn apply(name: &str, img: &mut Decoded) {
    match name {
        "greyscale" => {
            for px in img.rgba.chunks_exact_mut(4) {
                let g = (px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000;
                px[0] = g as u8;
                px[1] = g as u8;
                px[2] = g as u8;
            }
        }
        "sepia" => {
            for px in img.rgba.chunks_exact_mut(4) {
                let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
                px[0] = (r * 0.393 + g * 0.769 + b * 0.189).min(255.0) as u8;
                px[1] = (r * 0.349 + g * 0.686 + b * 0.168).min(255.0) as u8;
                px[2] = (r * 0.272 + g * 0.534 + b * 0.131).min(255.0) as u8;
            }
        }
        "invert" => {
            for px in img.rgba.chunks_exact_mut(4) {
                px[0] = 255 - px[0];
                px[1] = 255 - px[1];
                px[2] = 255 - px[2];
            }
        }
        "enhance" => enhance(img),
        "redeye" => redeye(img),
        _ => {}
    }
}

/// Auto-enhance: visible "kräftigere Farben" — mild contrast
/// S-curve plus a saturation boost. (A plain min/max stretch
/// does nothing on images that already span 0-255 — which is
/// most photos.)
fn enhance(img: &mut Decoded) {
    for px in img.rgba.chunks_exact_mut(4) {
        let avg = (px[0] as f32 + px[1] as f32 + px[2] as f32) / 3.0;
        for c in 0..3 {
            // Contrast: pull values away from mid grey by 12%.
            let v = (px[c] as f32 - 128.0) * 1.12 + 128.0;
            // Saturation: push each channel away from the luma mean
            // by 30% — colours come out noticeably stronger.
            let v = avg + (v - avg) * 1.30;
            px[c] = v.clamp(0.0, 255.0) as u8;
        }
    }
}

/// Red-eye reduction. A naive "any red pixel" pass repaints half
/// the photo; the real artifact is *strongly saturated red
/// inside a dark surround* (the pupil). Two checks: the pixel
/// must be very red, AND its ring neighbourhood must be dark —
/// red clothing on skin fails the second test.
fn redeye(img: &mut Decoded) {
    let (w, h) = (img.width as usize, img.height as usize);
    let src = img.rgba.clone();
    let at = |x: usize, y: usize| -> (u16, u16, u16) {
        let i = (y * w + x) * 4;
        (src[i] as u16, src[i + 1] as u16, src[i + 2] as u16)
    };
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = at(x, y);
            // Strict redness: bright red with weak other channels.
            if r < 150 || g > r / 2 || b > r / 2 {
                continue;
            }
            // Dark-surround check: mean brightness of a 4px ring —
            // a red eye sits inside the dark pupil/iris.
            let mut sum = 0u32;
            let mut n = 0u32;
            for dy in [-4i64, 4] {
                for dx in -4i64..=4 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let (r2, g2, b2) = at(nx as usize, ny as usize);
                        sum += r2 as u32 + g2 as u32 + b2 as u32;
                        n += 3;
                    }
                }
            }
            if n > 0 && sum / n < 90 {
                let i = (y * w + x) * 4;
                // Pull red down to the brighter other channel.
                img.rgba[i] = g.max(b) as u8;
            }
        }
    }
}
