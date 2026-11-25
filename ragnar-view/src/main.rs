// Copyright (C) Jörg Schröder
//! ragnar-view — Ragnar's desktop icons layer.
//!
//! One bottom layer-shell surface on the primary monitor carrying
//! ~/Desktop icons, Home, Trash and mounted volumes, plus the
//! desktop right-click menus. Replaces the legacy ragnar-desktop-view.

mod desktop;
mod dialogs;
mod fxcfg;
mod fxlabel;
mod i18n;
mod icon;
mod items;
mod layout;
mod menu;
mod menu_file;
mod prefs;
mod surfaces;

use gtk4::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "org.lieraos.ragnar.View";

fn main() {
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain("ragnar-view", "/usr/share/locale").ok();
    gettextrs::bind_textdomain_codeset("ragnar-view", "UTF-8").ok();
    gettextrs::textdomain("ragnar-view").ok();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|a| {
        // The surface must be see-through: only icons and menus draw
        // pixels, swaybg's wallpaper stays visible underneath.
        let css = gtk4::CssProvider::new();
        css.load_from_string(
            "window.desktop-surface { background-color: transparent; box-shadow: none; }
             .desktop-icon { border-radius: 8px; padding: 2px; }
             .desktop-icon label, .desktop-icon fxlabel { color: white; font-size: 0.9em; }
             .desktop-icon.selected { background-color: alpha(white, 0.22); }
             .desktop-icon.dragging { opacity: 0.7; }
             .rubber-band { border: 1px solid #3584e4; background-color: alpha(#3584e4, 0.18); border-radius: 2px; }
             .desktop-menu-canvas { background-color: alpha(white, 0.004); }",
        );
        if let Some(d) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &d,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        // One icon surface per monitor, kept alive for the app
        // lifetime; also shields every output's right-click from
        // the compositor's own root menu.
        surfaces::watch(a);
    });
    app.run();
}
