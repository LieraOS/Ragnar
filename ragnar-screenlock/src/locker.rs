// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-screenlock — lock command assembly
//
// Picks the locker binary and builds its full command line, including
// gtklock's config/style file resolution and swaylock's wallpaper
// argument — same resolution order as the legacy implementation.

use std::path::Path;

/// Which locker to run — first found wins (gtklock preferred).
pub fn which() -> Option<&'static str> {
    for app in ["gtklock", "swaylock"] {
        if gio::glib::find_program_in_path(app).is_some() {
            return Some(app);
        }
    }
    None
}

/// Full lock command line for the current session configuration.
pub fn command(picture_uri: &str) -> String {
    match which() {
        Some("gtklock") => gtklock_cmd(),
        Some("swaylock") => swaylock_cmd(picture_uri),
        _ => String::new(),
    }
}

/// gtklock -d plus -c config and -s style resolution:
/// ~/.config/ragnar-desktop → /etc/ragnar-desktop → /usr/share/ragnar-desktop
fn gtklock_cmd() -> String {
    let mut out = String::from("gtklock -d");
    if let Some(cfg) = first_existing("gtklock.ini") {
        out += &format!(" -c {cfg}");
    }
    if let Some(css) = first_existing("gtklock.css") {
        out += &format!(" -s {css}");
    }
    out
}

/// swaylock fallback with the configured wallpaper (or black).
fn swaylock_cmd(picture_uri: &str) -> String {
    let mut img = picture_uri.replace("file://", "");
    // swaylock uses ':' as output delimiter — escape it like swaylock
    // recommends.
    img = img.replace(':', "::");
    let arg = if img.is_empty() || !Path::new(&img).exists() {
        "-c 000000".to_string()
    } else {
        format!("-i {img}")
    };
    format!("swaylock -Fklf {arg}")
}

/// First existing Ragnar config file across the three search roots.
fn first_existing(name: &str) -> Option<String> {
    let user = format!(
        "{}/ragnar-desktop/{name}",
        glib::user_config_dir().display()
    );
    for cand in [
        user,
        format!("/etc/ragnar-desktop/{name}"),
        format!("/usr/share/ragnar-desktop/{name}"),
    ] {
        if Path::new(&cand).exists() {
            return Some(cand);
        }
    }
    None
}
