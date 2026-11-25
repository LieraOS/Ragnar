// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Classic log-file collector — the second layer of coverage for
// programs that bypass journald entirely (Xorg/Wayland, LightDM,
// installer logs). Lines containing error markers are collected;
// files unreadable by the current user are skipped silently.

use crate::entry::Entry;
use std::fs;
use std::path::{Path, PathBuf};

/// Files that only mirror journald content — scanning them would
/// duplicate every journal entry, so they are excluded on purpose.
const MIRRORED: &[&str] = &[
    "syslog",
    "kern.log",
    "messages",
    "daemon.log",
    "user.log",
    "auth.log",
    "debug",
    "dpkg.log",
    "alternatives.log",
    "fontconfig.log",
    "bootstrap.log",
    "faillog",
    "btmp",
    "wtmp",
    "lastlog",
    "Xorg.0.log.old",
];

/// Substrings that mark a line as an error (matched lowercase).
const MARKERS: &[&str] = &["error", "fatal", "crit", "failed", "failure", "panic"];

/// Collect error lines from all candidate log files.
pub fn collect() -> Vec<Entry> {
    let mut out = Vec::new();
    for path in candidate_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue; // unreadable for this user — skip
        };
        // Group under the path relative to /var/log so lightdm/
        // shows as "log:lightdm/seat0-greeter.log" style names.
        let group = format!("log:{}", display_name(&path));
        for line in text.lines() {
            let l = line.to_lowercase();
            if MARKERS.iter().any(|m| l.contains(m)) {
                out.push(Entry {
                    group: group.clone(),
                    timestamp: String::new(),
                    message: line.trim().to_string(),
                });
            }
        }
    }
    out
}

/// All log files worth scanning: /var/log/*.log (minus mirrored
/// journal duplicates), /var/log/lightdm/*, the installer logs and
/// the user's own Xorg logs under ~/.local/share/xorg.
fn candidate_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(dir) = fs::read_dir("/var/log") {
        for e in dir.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy();
            if name.ends_with(".log") && !MIRRORED.contains(&name.as_ref()) {
                files.push(p);
            }
        }
    }
    for dir in ["/var/log/lightdm", "/var/log/installer"] {
        collect_dir(dir, &mut files);
    }
    if let Ok(home) = std::env::var("HOME") {
        collect_dir(&format!("{home}/.local/share/xorg"), &mut files);
    }
    files
}

/// Add every readable regular file of a directory to the list.
fn collect_dir(dir: &str, files: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_file() {
                files.push(e.path());
            }
        }
    }
}

/// Short display name: strip /var/log/ or the home prefix.
fn display_name(p: &Path) -> String {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("/var/log/") {
        return rest.to_string();
    }
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = s.strip_prefix(&format!("{home}/")) {
            return format!("~/{rest}");
        }
    }
    s.to_string()
}
