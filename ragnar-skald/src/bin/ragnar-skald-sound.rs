// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-skald-sound — freedesktop event-sound player for LieraOS
// Copyright (C) 2026 LieraOS Project
//
// `ragnar-skald-sound <event>` plays the sound configured for that
// event in ragnar-settings' sounds.ini: resolves the sound name to
// a file in the active freedesktop theme and plays it through
// PipeWire with the event's own volume. Self-contained on purpose —
// event triggers must not load the whole audio library or GTK.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let Some(event) = std::env::args().nth(1) else {
        eprintln!("usage: ragnar-skald-sound <freedesktop-event-name>");
        std::process::exit(2);
    };
    play(&event);
}

/// Resolve the event to a file + volume, then play it.
fn play(event: &str) {
    let cfg = read_ini(&ini_path());
    if get(&cfg, "sounds", "enabled").as_deref() == Some("false") {
        return;
    }
    let theme = get(&cfg, "sounds", "theme").unwrap_or_else(|| "freedesktop".into());
    // "default" maps to the theme file named after the event itself.
    let name = match get(&cfg, "events", event).as_deref() {
        Some("none") => return,
        Some("default") | None => event.to_string(),
        Some(n) => n.to_string(),
    };
    let vol = get(&cfg, "volumes", event)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(80.0)
        / 100.0;
    let Some(file) = theme_file(&theme, &name) else {
        eprintln!("ragnar-skald-sound: no file for '{name}' in theme '{theme}'");
        return;
    };
    let _ = Command::new("pw-play")
        .args(["--volume", &format!("{vol:.2}")])
        .arg(file)
        .spawn();
}

/// First existing file for `name` in the theme's stereo dir.
fn theme_file(theme: &str, name: &str) -> Option<PathBuf> {
    let dir = format!("/usr/share/sounds/{theme}/stereo");
    for ext in ["oga", "ogg", "wav"] {
        let p = PathBuf::from(&dir).join(format!("{name}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// ragnar-settings' sound prefs.
fn ini_path() -> PathBuf {
    let mut p = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut d = PathBuf::from(std::env::var("HOME").unwrap_or_default());
            d.push(".config");
            d
        });
    p.push("ragnar-settings");
    p.push("sounds.ini");
    p
}

fn read_ini(path: &PathBuf) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut map: BTreeMap<String, BTreeMap<String, String>> = Default::default();
    let mut section = String::new();
    if let Ok(s) = std::fs::read_to_string(path) {
        for line in s.lines() {
            let l = line.trim();
            if l.starts_with('[') && l.ends_with(']') {
                section = l[1..l.len() - 1].to_string();
            } else if let Some((k, v)) = l.split_once('=') {
                map.entry(section.clone())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

fn get(m: &BTreeMap<String, BTreeMap<String, String>>, sec: &str, key: &str) -> Option<String> {
    m.get(sec).and_then(|s| s.get(key)).cloned()
}
