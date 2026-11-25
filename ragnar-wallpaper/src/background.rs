// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-wallpaper — org.gnome.desktop.background readers
//
// The legacy daemon resolved the keys through Gnome.BG; the relevant
// outcome is only picture-uri + picture-options (+ primary-color for
// the solid-color case), which we read directly.

use gio::prelude::*;
use gio::Settings;

const SCHEMA: &str = "org.gnome.desktop.background";

/// What the wallpaper layer should display right now.
#[derive(Clone)]
pub enum Choice {
    /// Image file + swaybg scaling mode (fill/fit/center/tile/stretch).
    Image { path: String, mode: &'static str },
    /// Solid color — options "none" or no/invalid uri (swaybg -c).
    SolidColor { color: String },
}

/// Reads the current settings and returns the choice to apply.
pub fn current() -> Choice {
    let s = Settings::new(SCHEMA);
    let uri = s.string("picture-uri").to_string();
    let path = uri.strip_prefix("file://").unwrap_or(&uri).to_string();
    let options = s.string("picture-options").to_string();

    // Gnome.BG marks color wallpapers by a null filename or placement
    // NONE — our equivalent: empty uri, missing file, or options=none.
    if options == "none" || path.is_empty() || !std::path::Path::new(&path).exists() {
        return Choice::SolidColor {
            color: s.string("primary-color").to_string(),
        };
    }
    Choice::Image {
        path,
        mode: mode(&options),
    }
}

/// Maps GNOME picture-options to swaybg scaling modes — same table
/// as the legacy get_swaybg_mode().
fn mode(options: &str) -> &'static str {
    match options {
        "wallpaper" => "tile",
        "centered" => "center",
        "scaled" => "fit",
        "stretched" => "stretch",
        // zoom + spanned have no exact swaybg equivalents; the fork
        // already approximated both with fill.
        _ => "fill",
    }
}

/// Re-runs the callback whenever a background key changes.
pub fn watch<F: Fn(Choice) + 'static>(cb: F) {
    // The Settings object must outlive the connection — a dropped
    // Settings disconnects the handler, which silently disabled
    // live wallpaper updates until now. Leaking one instance for
    // the session lifetime is the intentional fix.
    let settings: &'static Settings = Box::leak(Box::new(Settings::new(SCHEMA)));
    settings.connect_changed(None, move |_, key| {
        if ["picture-uri", "picture-options", "primary-color"].contains(&key) {
            // keys are re-read inside current()
            cb(current());
        }
    });
}
