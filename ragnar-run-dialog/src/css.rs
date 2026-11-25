// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-run-dialog — CSS providers
//
// Two providers, same convention as the other Ragnar tools:
//   1. the compiled-in default (data/run-dialog.css)
//   2. ~/.config/ragnar-desktop/run-dialog.css at USER priority —
//      how Ragnar Settings → Appearance (or a manual edit)
//      re-themes the overlay later.

use gtk4::{gdk, gio, CssProvider};

/// Path to the user theme, relative to the XDG config dir.
const USER_CSS: &str = "ragnar-desktop/run-dialog.css";

/// Install both providers on the default display; the user file
/// wins per rule when present.
pub fn load() {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let default = CssProvider::new();
    #[allow(deprecated)] // load_from_string/load_from_bytes need gtk4-rs ≥ 0.10
    default.load_from_data(include_str!("../data/run-dialog.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &default,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let user_file = dirs::config_dir().map(|d| d.join(USER_CSS));
    if let Some(path) = user_file.filter(|p| p.is_file()) {
        let user = CssProvider::new();
        user.load_from_file(&gio::File::for_path(path));
        gtk4::style_context_add_provider_for_display(
            &display,
            &user,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}
