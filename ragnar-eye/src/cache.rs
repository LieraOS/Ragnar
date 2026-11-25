// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — thumbnail disk cache.
//
// $XDG_CACHE_HOME/ragnar-eye/thumbs/<hash>.png keyed on
// path+mtime+size — stale entries are impossible because the key
// changes when the file does. Shared by the browser sidebar and
// the thumbnail strip.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Cache directory, created lazily.
pub fn dir() -> PathBuf {
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
        });
    let d = base.join("ragnar-eye").join("thumbs");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Content-stable cache key for a file.
fn key(path: &str) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    m.len().hash(&mut h);
    m.modified().ok().hash(&mut h);
    Some(format!("{:016x}.png", h.finish()))
}

/// Look up a cached thumbnail file.
pub fn get(path: &str) -> Option<PathBuf> {
    let p = dir().join(key(path)?);
    p.exists().then_some(p)
}

/// Store a rendered thumbnail PNG for `path`.
pub fn put(path: &str, png: &[u8]) {
    if let Some(k) = key(path) {
        let _ = std::fs::write(dir().join(k), png);
    }
}
