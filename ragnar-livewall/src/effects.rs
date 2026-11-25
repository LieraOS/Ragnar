// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-livewall — per-image effect configuration
//
// Reads ~/.config/ragnar-settings/livewall-effects.ini, written
// by the effects editor in ragnar-settings. One section per
// wallpaper (full image path; a basename section matches as a
// fallback). Regions are fractions of the image with y from the
// top; several regions per effect are separated by semicolons.
//
//   [/full/path/wallpaper.png]
//   aurora = true
//   aurora.region = 0.0,0.0,1.0,1.0
//   aurora.intensity = 1.0
//   stars = true
//   stars.region = 0.0,0.0,0.5,0.4 ; 0.6,0.05,0.4,0.3
//   stars.density = 0.7
//   stars.speed = 1.0
//   mist = true
//   mist.region = 0.0,0.55,1.0,0.3 ; 0.0,0.0,0.2,0.4
//   mist.opacity = 0.35
//   mist.drift = 1.0
//
// The same scheme applies to snow (.density .speed), rain
// (.strength .speed), fireflies (.density .speed) and clouds
// (.opacity .speed).

use std::path::{Path, PathBuf};

/// Up to four regions per effect — the shader loops a fixed
/// vec4[4] and stops at the region count.
pub const MAX_REGIONS: usize = 4;

/// Flat, uniform-ready effect set. Rects are x,y,w,h fractions
/// (y from top). The *-tuples carry a count/enabled flag as
/// their last component so a disabled effect is just zeros to
/// the shader.
#[derive(Clone)]
pub struct Fx {
    pub aurora_rect: [f32; 4],
    /// intensity, enabled
    pub aurora: [f32; 2],
    pub stars_rects: [[f32; 4]; MAX_REGIONS],
    /// density, speed, region count
    pub stars: [f32; 3],
    pub mist_rects: [[f32; 4]; MAX_REGIONS],
    /// opacity, drift, region count
    pub mist: [f32; 3],
    pub snow_rects: [[f32; 4]; MAX_REGIONS],
    /// density, fall speed, region count
    pub snow: [f32; 3],
    pub rain_rects: [[f32; 4]; MAX_REGIONS],
    /// strength, speed, region count
    pub rain: [f32; 3],
    pub fly_rects: [[f32; 4]; MAX_REGIONS],
    /// density, drift speed, region count
    pub fly: [f32; 3],
    pub cloud_rects: [[f32; 4]; MAX_REGIONS],
    /// opacity, speed, region count
    pub cloud: [f32; 3],
}

impl Default for Fx {
    /// No configuration: the aurora runs in its classic
    /// full-image band (the look that was field-approved),
    /// stars and mist stay off.
    fn default() -> Self {
        Self {
            aurora_rect: [0.0, 0.0, 1.0, 1.0],
            aurora: [1.0, 1.0],
            stars_rects: [[0.0; 4]; MAX_REGIONS],
            stars: [0.7, 1.0, 0.0],
            mist_rects: [[0.0; 4]; MAX_REGIONS],
            mist: [0.35, 1.0, 0.0],
            snow_rects: [[0.0; 4]; MAX_REGIONS],
            snow: [0.7, 1.0, 0.0],
            rain_rects: [[0.0; 4]; MAX_REGIONS],
            rain: [0.6, 1.0, 0.0],
            fly_rects: [[0.0; 4]; MAX_REGIONS],
            fly: [0.6, 1.0, 0.0],
            cloud_rects: [[0.0; 4]; MAX_REGIONS],
            cloud: [0.4, 1.0, 0.0],
        }
    }
}

/// Path of the shared effects file.
pub fn ini_path() -> PathBuf {
    let mut p = dirs_config();
    p.push("ragnar-settings");
    p.push("livewall-effects.ini");
    p
}

fn dirs_config() -> PathBuf {
    if let Ok(c) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(c);
    }
    let mut p = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    p.push(".config");
    p
}

/// Load the Fx for `image` — matches the section whose name is
/// the full path, else one equal to the file's basename.
pub fn load(image: &Path) -> Fx {
    let mut fx = Fx::default();
    let Ok(text) = std::fs::read_to_string(ini_path()) else {
        return fx;
    };
    let base = image.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let full = image.to_string_lossy();

    // Collect the matching section's pairs first — `enabled`
    // must gate the region count, so evaluation order cannot
    // depend on key order inside the file.
    let mut in_section = false;
    let mut kv: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            let name = line.trim_matches(|c| c == '[' || c == ']');
            in_section = name == full.as_ref() || name == base;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            kv.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let get = |k: &str| kv.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str());
    if let Some(v) = get("aurora") {
        fx.aurora[1] = bool_f(v);
    }
    if let Some(v) = get("aurora.region") {
        if let Some(r) = rects(v).first() {
            fx.aurora_rect = *r;
        }
    }
    if let Some(v) = get("aurora.intensity") {
        fx.aurora[0] = parse_f(v, 1.0).clamp(0.0, 2.0);
    }
    // Multi-region effects, all sharing one parse pattern:
    // (prefix, param1 key, param2 key, default1, default2)
    let spec = |prefix: &str, k1: &str, k2: &str,
                    d1: f32, d2: f32|
     -> ([[f32; 4]; MAX_REGIONS], [f32; 3]) {
        let mut rs = [[0.0; 4]; MAX_REGIONS];
        if let Some(v) = get(&format!("{prefix}.region")) {
            rs = rects(v);
        }
        // Count = enabled ? number of configured regions : 0.
        let n = if get(prefix).is_some_and(bool) {
            rs.iter().filter(|r| r[2] > 0.0).count() as f32
        } else {
            0.0
        };
        let p1 = get(&format!("{prefix}.{k1}"))
            .map(|v| parse_f(v, d1))
            .unwrap_or(d1)
            .clamp(0.0, if d1 >= 1.5 { 2.0 } else { 1.0 });
        let p2 = get(&format!("{prefix}.{k2}"))
            .map(|v| parse_f(v, d2))
            .unwrap_or(d2)
            .clamp(0.0, 4.0);
        (rs, [p1, p2, n])
    };
    (fx.stars_rects, fx.stars) = spec("stars", "density", "speed", 0.7, 1.0);
    (fx.mist_rects, fx.mist) = spec("mist", "opacity", "drift", 0.35, 1.0);
    (fx.snow_rects, fx.snow) = spec("snow", "density", "speed", 0.7, 1.0);
    (fx.rain_rects, fx.rain) = spec("rain", "strength", "speed", 0.6, 1.0);
    (fx.fly_rects, fx.fly) = spec("fireflies", "density", "speed", 0.6, 1.0);
    (fx.cloud_rects, fx.cloud) = spec("clouds", "opacity", "speed", 0.4, 1.0);
    fx
}

fn bool(v: &str) -> bool {
    matches!(v, "true" | "yes" | "on" | "1")
}

fn bool_f(v: &str) -> f32 {
    if bool(v) { 1.0 } else { 0.0 }
}

fn parse_f(v: &str, def: f32) -> f32 {
    v.parse().unwrap_or(def)
}

/// "x,y,w,h ; x,y,w,h ; …" — up to MAX_REGIONS rects.
fn rects(v: &str) -> [[f32; 4]; MAX_REGIONS] {
    let mut out = [[0.0; 4]; MAX_REGIONS];
    for (i, part) in v.split(';').take(MAX_REGIONS).enumerate() {
        for (j, n) in part.split(',').take(4).enumerate() {
            out[i][j] = n.trim().parse().unwrap_or(0.0);
        }
    }
    out
}
