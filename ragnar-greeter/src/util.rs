// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — small shared helpers
//
// Texture loading (with the artwork search path) and the
// deterministic Park-Miller generator the Plymouth script's
// random choices were modelled on.

use gtk4::Picture;

/// Artwork files installed to /usr/share/ragnar-greeter/artwork/.
const ART: &str = "/usr/share/ragnar-greeter/artwork";

/// Load an artwork picture; missing files degrade to an empty
/// widget so the greeter still runs on an unpacked tree. Search
/// order: RAGNAR_GREETER_ARTWORK env dir, the installed path,
/// then the source tree so `--preview` works from `cargo run`.
pub fn picture(name: &str) -> Picture {
    let env_dir = std::env::var("RAGNAR_GREETER_ARTWORK").unwrap_or_default();
    for dir in [env_dir.as_str(), ART, "data/artwork"] {
        if dir.is_empty() {
            continue;
        }
        if let Some(t) = gdk4_texture(&format!("{dir}/{name}")) {
            return Picture::for_paintable(&t);
        }
    }
    Picture::new()
}

/// Texture from a file path (PNG only here — artwork is ours).
fn gdk4_texture(path: &str) -> Option<gtk4::gdk::Texture> {
    gtk4::gdk::Texture::from_filename(path).ok()
}

/// Park-Miller minimal standard (same generator the Plymouth
/// script used — deterministic, no crate needed for two picks
/// per strike).
pub fn park_miller(state: &mut u64) -> f64 {
    *state = (*state * 48271) % 2147483647;
    *state as f64 / 2147483647.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn park_miller_stays_in_unit_range() {
        let mut s = 987654321u64;
        for _ in 0..10_000 {
            let v = park_miller(&mut s);
            assert!(v > 0.0 && v < 1.0, "out of range: {v}");
        }
    }

    #[test]
    fn park_miller_is_deterministic() {
        let (mut a, mut b) = (12345u64, 12345u64);
        for _ in 0..100 {
            assert_eq!(park_miller(&mut a), park_miller(&mut b));
        }
    }
}
