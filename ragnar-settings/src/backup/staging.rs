// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — backup: staging & filesystem helpers
// Copyright (C) 2026 LieraOS Project
//
// The mechanical half of backup/restore: unique temp dirs,
// recursive copy preserving symlinks, and the pkexec script that
// reinstalls system files. Manifest and orchestration live in
// backup/mod.rs.

use std::path::{Path, PathBuf};

/// Unique staging dir inside the system temp directory.
pub fn staging_dir(tag: &str) -> PathBuf {
    let p = gtk4::glib::tmp_dir().join(format!("ragnar-settings-{}-{}", tag, std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Copy a file, symlink or directory tree `src` → `dst`, creating
/// parents. Returns false when `src` does not exist (skipped).
pub fn copy_entry(src: &Path, dst: &Path, is_dir: bool) -> bool {
    if src.is_symlink() {
        if let (Some(parent), Ok(target)) = (dst.parent(), std::fs::read_link(src)) {
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::remove_file(dst);
            let _ = std::os::unix::fs::symlink(&target, dst);
            return true;
        }
        return false;
    }
    if is_dir {
        let Ok(rd) = src.read_dir() else { return false };
        for e in rd.flatten() {
            copy_entry(&e.path(), &dst.join(e.file_name()), e.path().is_dir());
        }
        return true;
    }
    if !src.is_file() {
        return false;
    }
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::copy(src, dst).is_ok()
}

/// Shell script that copies every staged system file to its real
/// path — run via pkexec so one prompt covers all files. `etc` is
/// the archive's `etc/` staging dir whose relative layout mirrors
/// the absolute destination (etc/apt/… → /etc/apt/…).
/// Returns None when the backup contains no system files.
pub fn build_sys_script(etc: &Path) -> Option<String> {
    let mut lines = vec!["set -e".to_string()];
    collect_tree(etc, Path::new("/"), &mut lines);
    (lines.len() > 1).then(|| lines.join("\n"))
}

/// Walk a staged tree; each file/dir/symlink becomes one command.
fn collect_tree(src_dir: &Path, dst_dir: &Path, lines: &mut Vec<String>) {
    let Ok(rd) = src_dir.read_dir() else { return };
    for e in rd.flatten() {
        let (s, d) = (e.path(), dst_dir.join(e.file_name()));
        if s.is_symlink() {
            if let Ok(t) = std::fs::read_link(&s) {
                lines.push(format!("ln -sf '{}' '{}'", t.display(), d.display()));
            }
        } else if s.is_dir() {
            lines.push(format!("mkdir -p '{}'", d.display()));
            collect_tree(&s, &d, lines);
        } else {
            lines.push(format!("cp -f '{}' '{}'", s.display(), d.display()));
        }
    }
}
