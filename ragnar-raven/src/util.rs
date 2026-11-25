// Copyright (C) Jörg Schröder
//! Small shared helpers.

use gtk4::glib;
use std::fs;
use std::path::{Path, PathBuf};

/// Path of Raven's shared settings.ini — written by the taskbar
/// calendar menu and ragnar-settings, read and watched (watch.rs)
/// here.
pub fn settings_path() -> PathBuf {
    glib::user_config_dir()
        .join("ragnar-raven")
        .join("settings.ini")
}

/// Locate `key` in `section` of ini `text` — the pure core that
/// ini_get wraps around settings.ini, split out for unit tests.
fn ini_lookup(text: &str, section: &str, key: &str) -> Option<String> {
    let want = format!("[{section}]");
    let mut in_sec = false;
    let mut out = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_sec = t == want;
        } else if in_sec {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == key {
                    out = Some(v.trim().to_string());
                }
            }
        }
    }
    out
}

/// Read one ini section into key→value, last write wins.
///
/// Deliberately lenient: GKeyFile rejects the ENTIRE file on a
/// single malformed line, and this file is edited by other tools —
/// a corrupted section elsewhere must not blank out our settings
/// (a broken `[notifications` header once silently disabled every
/// key we read).
pub fn ini_get(section: &str, key: &str) -> Option<String> {
    ini_lookup(
        &std::fs::read_to_string(settings_path()).ok()?,
        section,
        key,
    )
}

/// `key` parsed as integer, None when absent or unparsable.
pub fn ini_int(section: &str, key: &str) -> Option<i64> {
    ini_get(section, key)?.parse().ok()
}

/// `key` parsed as boolean ("true"/"1" vs "false"/"0"); anything
/// else is None so callers fall back to their own defaults.
pub fn ini_bool(section: &str, key: &str) -> Option<bool> {
    match ini_get(section, key)?.as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// Pure repair core: malformed ini text in, normalized text out —
/// or None when nothing was wrong (split out for unit tests).
///
/// Rules: headers `[name` without the bracket still open a section
/// (named `name`); duplicate headers merge into the first section
/// of that name; duplicate keys keep their last write; comment and
/// non-key lines are dropped — the file is a settings store, not
/// a document.
fn repaired_ini(text: &str) -> Option<String> {
    let mut order: Vec<String> = Vec::new();
    let mut secs: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut cur = String::new();
    let mut dirty = false;
    for raw in text.lines() {
        let t = raw.trim();
        if t.starts_with('[') {
            // "[name" without ']' is still a section header — just
            // a malformed one that must be repaired.
            let name = t
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            if !t.ends_with(']') || name.is_empty() {
                dirty = true;
            }
            if name.is_empty() {
                cur.clear();
                continue;
            }
            if !seen.insert(name.clone()) {
                dirty = true; // duplicate section header
            }
            if !secs.contains_key(&name) {
                order.push(name.clone());
                secs.insert(name.clone(), Vec::new());
            }
            cur = name;
        } else if let Some((k, v)) = t.split_once('=') {
            if cur.is_empty() {
                continue;
            }
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            let list = secs.get_mut(&cur).unwrap();
            list.retain(|(ek, _)| *ek != k);
            list.push((k, v));
        }
    }
    if !dirty {
        return None;
    }
    let mut out = String::new();
    for (i, name) in order.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("[{name}]\n"));
        for (k, v) in &secs[name] {
            out.push_str(&format!("{k} = {v}\n"));
        }
    }
    Some(out)
}

/// One-shot repair pass over settings.ini.
///
/// An earlier ragnar-settings build wrote section headers without
/// the closing bracket (`[notifications`) — strict parsers then
/// rejected the whole file. Rewrites the file atomically only when
/// malformed or duplicated headers were actually found.
pub fn sanitize_settings() {
    let path = settings_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    if let Some(out) = repaired_ini(&text) {
        write_atomic(&path, &out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The actual corruption observed in the wild: ragnar-settings
    /// appended `[notifications` sections on every sound write.
    const CORRUPT: &str = "[notifications\nsound = default\n[notifications\nsound_volume = 91\n[notifications]\nduration_s = 15\nhistory_days = 0\n";

    #[test]
    fn lookup_reads_valid_section_despite_broken_ones() {
        assert_eq!(
            ini_lookup(CORRUPT, "notifications", "history_days").as_deref(),
            Some("0")
        );
    }

    #[test]
    fn lookup_returns_none_for_absent() {
        assert_eq!(ini_lookup(CORRUPT, "notifications", "sound"), None);
        assert_eq!(ini_lookup("[a]\nx = 1\n", "a", "y"), None);
    }

    #[test]
    fn repair_merges_broken_and_duplicate_sections() {
        let out = repaired_ini(CORRUPT).expect("file is dirty");
        assert_eq!(
            out,
            "[notifications]\nsound = default\nsound_volume = 91\nduration_s = 15\nhistory_days = 0\n"
        );
    }

    #[test]
    fn repair_keeps_later_sections() {
        let fixed = repaired_ini("[a]\nx = 1\n[a\ny = 2\n").unwrap();
        assert_eq!(fixed, "[a]\nx = 1\ny = 2\n");
    }

    #[test]
    fn repair_leaves_clean_files_alone() {
        assert!(repaired_ini("[a]\nx = 1\n[b]\ny = 2\n").is_none());
    }
}

/// Directory for per-appointment sound copies — user-picked files
/// are copied here so the .ics keeps working after the original is
/// moved or deleted.
pub fn sounds_dir() -> PathBuf {
    let dir = glib::user_data_dir().join("ragnar-raven").join("sounds");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Copy a user-picked audio file into the sounds dir, named
/// `<uid>-<basename>`; returns the stored path. A `src` that is
/// already inside the sounds dir is returned unchanged (an edit
/// saving the same file must not copy it onto itself).
pub fn copy_sound(uid: &str, src: &Path) -> Option<PathBuf> {
    if src.starts_with(sounds_dir()) {
        return Some(src.to_path_buf());
    }
    let name = src.file_name()?.to_string_lossy().replace(['/', ' '], "_");
    let dest = sounds_dir().join(format!("{uid}-{name}"));
    fs::copy(src, &dest).ok().map(|_| dest)
}

/// Delete a stored sound copy — files outside the sounds dir are
/// never ours to remove.
pub fn drop_sound(path: &Path) {
    if path.starts_with(sounds_dir()) {
        let _ = fs::remove_file(path);
    }
}

/// Write `data` to `path` atomically: the payload lands in a sibling
/// temp file first and is renamed over the target, so a crash can
/// never leave a half-written store behind (rename is atomic on the
/// same filesystem — the temp file lives in the same directory).
pub fn write_atomic(path: &Path, data: &str) {
    let tmp = match path.file_name() {
        Some(name) => path.with_file_name(format!("{}.tmp", name.to_string_lossy())),
        None => return,
    };
    if fs::write(&tmp, data).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}
