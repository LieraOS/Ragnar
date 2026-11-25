//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Persistent settings (~/.config/ragnar-edit/settings.ini, GKeyFile).

use std::path::PathBuf;

#[derive(Clone)]
pub struct Settings {
    pub dark: bool,
    pub font: String,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub autosave_secs: u32,
    pub lang: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark: true,
            font: "Monospace 11".into(),
            word_wrap: true,
            line_numbers: true,
            autosave_secs: 0,
            lang: String::new(),
        }
    }
}

fn path() -> PathBuf {
    let mut p = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()));
    p.push(".config/ragnar-edit/settings.ini");
    p
}

pub fn load() -> Settings {
    let kf = gtk4::glib::KeyFile::new();
    let mut s = Settings::default();
    if kf
        .load_from_file(path(), gtk4::glib::KeyFileFlags::NONE)
        .is_err()
    {
        return s;
    }
    let g = "editor";
    s.dark = kf.boolean(g, "dark").unwrap_or(true);
    s.font = kf
        .string(g, "font")
        .map(|x| x.to_string())
        .unwrap_or(s.font);
    s.word_wrap = kf.boolean(g, "word_wrap").unwrap_or(true);
    s.line_numbers = kf.boolean(g, "line_numbers").unwrap_or(true);
    s.autosave_secs = kf.integer(g, "autosave_secs").unwrap_or(0).max(0) as u32;
    s.lang = kf
        .string(g, "language")
        .map(|x| x.to_string())
        .unwrap_or_default();
    s
}

pub fn save(s: &Settings) {
    let kf = gtk4::glib::KeyFile::new();
    let g = "editor";
    kf.set_boolean(g, "dark", s.dark);
    kf.set_string(g, "font", &s.font);
    kf.set_boolean(g, "word_wrap", s.word_wrap);
    kf.set_boolean(g, "line_numbers", s.line_numbers);
    kf.set_integer(g, "autosave_secs", s.autosave_secs as i32);
    kf.set_string(g, "language", &s.lang);
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = kf.save_to_file(p);
}
