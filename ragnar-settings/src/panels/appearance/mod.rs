//! Appearance settings panel — ported from the old ragnar-settings-gui.
//! Sub-pages: wallpaper, themes, style, icons, fonts, cursor.

mod common;
mod cursor;
mod fonts;
mod icons;
mod root;
mod style;
mod themes;
mod wallpaper;
mod wallpaper_map;

use gtk4::Widget;

/// Build the appearance panel widget.
pub fn build() -> Widget {
    root::create_page()
}
