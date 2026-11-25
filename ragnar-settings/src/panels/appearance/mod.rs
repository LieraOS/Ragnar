// Copyright (C) Jörg Schröder
//! Appearance settings panel — ported from the old ragnar-settings-gui.
//! Sub-pages: wallpaper, themes, style, icons, fonts, cursor.

mod common;
mod cursor;
pub mod font_fx;
mod fonts;
mod fx_preview;
mod fx_preview_gl;
mod icons;
mod root;
mod style;
mod themes;
mod wallpaper;
mod wallpaper_anim;
mod wallpaper_fx;
mod wallpaper_fx_ini;
mod wallpaper_fx_row;
mod wallpaper_map;

use gtk4::Widget;

/// Build the appearance panel widget.
pub fn build() -> Widget {
    root::create_page()
}
