// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — generic INI helper for embedded panels
// Copyright (C) 2026 LieraOS Project
//
// Minimal section/key read and a write_key() that preserves every
// unknown line, section and comment — the files we touch
// (gammastep's config.ini, future per-daemon settings) are also
// edited by power users by hand, so dropping their keys is a bug.
// Every panel that touches these files goes through this module —
// private copies were removed after one produced malformed headers.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// A parsed INI file: (section, key) → value.
pub type IniMap = BTreeMap<(String, String), String>;

/// Read `path` into (section, key) → value. Lines without a
/// section, comments (`#`, `;`) and blank lines are ignored; a
/// missing file yields an empty map — callers layer their own
/// defaults on top.
pub fn read(path: &Path) -> IniMap {
    let mut map = IniMap::new();
    let mut section = String::new();
    if let Ok(s) = std::fs::read_to_string(path) {
        for line in s.lines() {
            let t = line.trim();
            if t.starts_with('[') && t.ends_with(']') {
                section = t[1..t.len() - 1].trim().to_string();
            } else if let Some((k, v)) = t.split_once('=') {
                if !section.is_empty() && !k.trim().is_empty() {
                    map.insert(
                        (section.clone(), k.trim().to_string()),
                        v.trim().to_string(),
                    );
                }
            }
        }
    }
    map
}

/// Fetch one key from a map returned by [`read`].
pub fn get(map: &IniMap, section: &str, key: &str) -> Option<String> {
    map.get(&(section.to_string(), key.to_string())).cloned()
}

/// Write one `key=value` line into `section` of the INI file at
/// `path`, preserving every other line (comments, unknown keys,
/// foreign sections). Replaces the line in place when the key
/// exists; otherwise appends it to the section — or appends a new
/// `[section]` block at EOF when the section is missing entirely.
/// Keys are written compactly (`key=value`, no spaces) because some
/// consumers — gammastep's config parser among them — do not trim
/// whitespace around `=` and would silently ignore spaced keys.
/// Parent directories are created as needed.
pub fn write_key(path: &Path, section: &str, key: &str, value: &str) -> io::Result<()> {
    let target = format!("[{section}]");
    let mut lines: Vec<String> = std::fs::read_to_string(path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();

    // First pass: replace the existing key inside the target section.
    let mut in_section = false;
    let mut section_end: Option<usize> = None;
    let mut replaced = false;
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_section && !replaced && section_end.is_none() {
                // Last index still inside our section.
                section_end = Some(i);
            }
            in_section = t == target;
            continue;
        }
        if in_section && !t.starts_with('#') && !t.starts_with(';') {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    *line = format!("{key}={value}");
                    replaced = true;
                }
            }
        }
    }
    if in_section && !replaced && section_end.is_none() {
        section_end = Some(lines.len());
    }

    if !replaced {
        match section_end {
            // Key missing inside an existing section.
            Some(i) => lines.insert(i, format!("{key}={value}")),
            // Section missing entirely.
            None => {
                if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
                    lines.push(String::new());
                }
                lines.push(target);
                lines.push(format!("{key}={value}"));
            }
        }
    }

    if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, lines.join("\n") + "\n")
}

/// Convenience for tests and callers that build the whole map of
/// keys belonging to one section.
#[allow(dead_code)]
pub fn section_values(map: &IniMap, section: &str) -> BTreeMap<String, String> {
    map.iter()
        .filter(|((s, _), _)| s == section)
        .map(|((_, k), v)| (k.clone(), v.clone()))
        .collect()
}

/// `~/.config/<sub>` — kept here so panels do not repeat the
/// config-dir plumbing.
#[allow(dead_code)]
pub fn config_path(sub: &str) -> PathBuf {
    glib_config_dir().join(sub)
}

/// glib's user config dir without pulling gtk4 into pure-logic
/// callers — mirrors g_get_user_config_dir().
fn glib_config_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(p);
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Round trip: unknown sections/keys/comments survive a
    /// write_key() untouched.
    #[test]
    fn write_key_preserves_unknown_lines() {
        let dir = std::env::temp_dir().join(format!("ragset-ini-{}", std::process::id()));
        let path = dir.join("config.ini");
        let original = "# user comment\n\
                        [general]\n\
                        temp-day=6500\n\
                        ; another comment\n\
                        gamma=0.8\n\
                        \n\
                        [other]\n\
                        keep=me\n";
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, original).unwrap();

        write_key(&path, "general", "temp-night", "4000").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# user comment"));
        assert!(text.contains("temp-day=6500"));
        assert!(text.contains("; another comment"));
        assert!(text.contains("gamma=0.8"));
        assert!(text.contains("[other]"));
        assert!(text.contains("keep=me"));
        assert!(text.contains("temp-night=4000"));

        // Update in place, not duplicated.
        write_key(&path, "general", "temp-night", "3500").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("temp-night").count(), 1);
        assert!(text.contains("temp-night=3500"));

        let map = read(&path);
        assert_eq!(get(&map, "general", "temp-night").as_deref(), Some("3500"));
        assert_eq!(get(&map, "general", "gamma").as_deref(), Some("0.8"));
        assert_eq!(get(&map, "other", "keep").as_deref(), Some("me"));

        // New section is appended, not merged.
        write_key(&path, "new", "k", "v").unwrap();
        let map = read(&path);
        assert_eq!(get(&map, "new", "k").as_deref(), Some("v"));
        let _ = fs::remove_dir_all(&dir);
    }
}
