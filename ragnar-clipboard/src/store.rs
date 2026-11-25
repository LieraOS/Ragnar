// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-clipboard — on-disk clipboard history.
//
// The disk is the source of truth: every captured selection is
// written to $XDG_DATA_HOME/ragnar-clipboard/ as one index line plus
// one blob file per stored MIME type. The daemon appends, the picker
// reads, nothing needs to be held in memory.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Maximum bytes stored per MIME type — larger offers are skipped.
const MAX_BLOB: usize = 8 * 1024 * 1024;
/// Maximum entries kept in the history.
pub const MAX_ENTRIES: usize = 100;

/// MIME types worth persisting, in preference order. Only types a
/// clipboard manager can re-provide verbatim are listed.
pub const WANTED_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
    "TEXT",
    "text/html",
    "image/png",
];

/// Password-manager hint — entries advertising it are never stored.
const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";

/// One stored clipboard entry.
#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    /// Unique id — timestamp plus sequence, also the blob directory.
    pub id: String,
    /// Unix timestamp of capture.
    pub ts: u64,
    /// Short one-line preview for the picker.
    pub preview: String,
    /// MIME type → blob file name (inside blobs/<id>/).
    pub mimes: Vec<(String, String)>,
    /// Content hash of the canonical payload — deduplication key.
    pub hash: u64,
}

/// Base directory for the store.
pub fn dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share")
        });
    base.join("ragnar-clipboard")
}

/// Sanitize a MIME type into a safe blob file name.
fn blob_name(mime: &str) -> String {
    mime.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Hash the canonical payload (first wanted MIME that was captured).
fn hash_of(mimes: &[(String, Vec<u8>)]) -> u64 {
    let mut h = DefaultHasher::new();
    for want in WANTED_MIMES {
        if let Some((_, data)) = mimes.iter().find(|(m, _)| m == want) {
            data.hash(&mut h);
            return h.finish();
        }
    }
    0
}

/// Short preview text for the picker list.
fn preview_of(mimes: &[(String, Vec<u8>)]) -> String {
    for want in WANTED_MIMES {
        if let Some((_, data)) = mimes.iter().find(|(m, _)| m == want) {
            if want.starts_with("text") || *want == "UTF8_STRING" || *want == "STRING" {
                let text = String::from_utf8_lossy(data);
                let line = text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim();
                return line.chars().take(120).collect();
            }
            return format!("[{}]", want);
        }
    }
    String::new()
}

/// Load all entries, newest first.
pub fn load() -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Ok(content) = fs::read_to_string(dir().join("index.jsonl")) {
        for line in content.lines() {
            if let Ok(e) = serde_json::from_str::<Entry>(line) {
                entries.push(e);
            }
        }
    }
    entries.reverse();
    entries
}

/// Rewrite the index file from the given entries (oldest first).
fn write_index(entries: &[Entry]) {
    let path = dir().join("index.jsonl");
    let mut f = match fs::File::create(&path) {
        Ok(f) => f,
        Err(_) => return,
    };
    for e in entries {
        if let Ok(line) = serde_json::to_string(e) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// True when the offered MIME list advertises sensitive content.
pub fn is_sensitive(offered: &[String]) -> bool {
    offered.iter().any(|m| m == PASSWORD_HINT)
}

/// Persist one captured selection. `mimes` holds the received bytes
/// for every wanted MIME type the offer provided. Returns the stored
/// entry, or None when nothing wanted was offered or the newest
/// entry already has the same payload.
pub fn store(mimes: Vec<(String, Vec<u8>)>) -> Option<Entry> {
    let useful: Vec<(String, Vec<u8>)> = mimes
        .into_iter()
        .filter(|(m, d)| !d.is_empty() && WANTED_MIMES.contains(&m.as_str()))
        .collect();
    if useful.is_empty() {
        return None;
    }
    let hash = hash_of(&useful);
    let mut entries = load();
    if entries.first().map(|e| e.hash) == Some(hash) {
        return None;
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("{ts}-{:04x}", hash & 0xffff);
    let blob_dir = dir().join("blobs").join(&id);
    let mut stored_mimes = Vec::new();
    for (mime, data) in &useful {
        if data.len() > MAX_BLOB {
            continue;
        }
        let name = blob_name(mime);
        if fs::create_dir_all(&blob_dir).is_ok()
            && fs::write(blob_dir.join(&name), data).is_ok()
        {
            stored_mimes.push((mime.clone(), name));
        }
    }
    if stored_mimes.is_empty() {
        return None;
    }

    let entry = Entry {
        id,
        ts,
        preview: preview_of(&useful),
        mimes: stored_mimes,
        hash,
    };
    entries.insert(0, entry.clone());

    // Trim history: drop the oldest entries and their blobs.
    while entries.len() > MAX_ENTRIES {
        if let Some(old) = entries.pop() {
            let _ = fs::remove_dir_all(dir().join("blobs").join(&old.id));
        }
    }
    entries.reverse();
    write_index(&entries);
    Some(entry)
}

/// Read the stored bytes for one MIME type of an entry.
pub fn blob(entry: &Entry, mime: &str) -> Option<Vec<u8>> {
    entry
        .mimes
        .iter()
        .find(|(m, _)| m == mime)
        .and_then(|(_, name)| {
            let mut buf = Vec::new();
            fs::File::open(dir().join("blobs").join(&entry.id).join(name))
                .ok()?
                .read_to_end(&mut buf)
                .ok()?;
            Some(buf)
        })
}

/// Delete the whole history.
pub fn wipe() {
    let _ = fs::remove_dir_all(dir());
}

/// Read at most `max` bytes from a pipe reader (clipboard offer).
pub fn read_capped<R: Read>(r: R, max: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = r.take(max as u64).read_to_end(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Redirect the store into a throwaway directory for this test.
    /// Every helper reads XDG_DATA_HOME lazily, so setting it once
    /// before exercising the store covers load/store/wipe alike.
    fn sandbox() -> PathBuf {
        let base = std::env::temp_dir()
            .join(format!("ragnar-clipboard-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &base);
        base
    }

    #[test]
    fn sensitive_hint_detected() {
        assert!(is_sensitive(&[PASSWORD_HINT.to_string()]));
        assert!(is_sensitive(&[
            "text/plain".to_string(),
            PASSWORD_HINT.to_string(),
        ]));
        assert!(!is_sensitive(&["text/plain".to_string()]));
    }

    #[test]
    fn read_capped_limits_input() {
        let data = vec![b'x'; 4096];
        assert_eq!(read_capped(&data[..], 16).len(), 16);
        assert_eq!(read_capped(&data[..], 8192).len(), 4096);
    }

    #[test]
    fn store_dedup_load_blob_wipe() {
        let base = sandbox();
        wipe();
        let text = |s: &str| {
            vec![("text/plain;charset=utf-8".to_string(), s.as_bytes().to_vec())]
        };

        // First capture is stored with a readable preview.
        let e1 = store(text("hello world")).expect("first entry stored");
        assert_eq!(e1.preview, "hello world");
        assert_eq!(load().len(), 1);

        // Identical payload right after is deduplicated.
        assert!(store(text("hello world")).is_none());
        assert_eq!(load().len(), 1);

        // New payload creates a second entry; blobs round-trip.
        let e2 = store(text("second entry")).expect("second entry stored");
        assert_eq!(load().len(), 2);
        assert_eq!(load()[0].id, e2.id, "newest entry first");
        assert_eq!(
            blob(&e2, "text/plain;charset=utf-8").as_deref(),
            Some(b"second entry".as_slice())
        );

        // Unwanted MIME types alone never produce an entry.
        assert!(store(vec![("application/x-moz-file".to_string(), vec![1])]).is_none());

        wipe();
        assert!(load().is_empty());
        let _ = fs::remove_dir_all(base);
    }
}
