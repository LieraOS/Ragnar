// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings-gui — appearance module: shared helpers
// Copyright (C) 2026 LieraOS Project
//
// Directory enumeration for themes/icons/cursors, archive install,
// and small row builders shared by the appearance sub-pages.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Directories containing GTK themes: system + user.
pub fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/themes")];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".themes"));
        dirs.push(home.join(".local/share/themes"));
    }
    dirs
}

/// Directories containing icon/cursor themes: system + user.
pub fn icon_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/icons")];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".icons"));
        dirs.push(home.join(".local/share/icons"));
    }
    dirs
}

/// List theme names in the given dirs that contain `marker` relative
/// (e.g. "gtk-3.0/gtk.css" for GTK themes, "index.theme" for icons).
/// `exclude` filters names (e.g. cursor-only dirs for the icon list).
pub fn list_named(dirs: &[PathBuf], marker: &str, exclude: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == exclude {
                continue;
            }
            if e.path().join(marker).exists() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// List GTK themes: a directory with gtk-3.0 or gtk-4.0 content.
pub fn list_gtk_themes() -> Vec<String> {
    let mut names = list_named(&theme_dirs(), "index.theme", "cursors");
    // Also accept themes that only ship gtk-3.0/gtk.css without index.theme.
    for dir in theme_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !e.path().is_dir() || names.contains(&name) {
                continue;
            }
            if e.path().join("gtk-3.0").exists() || e.path().join("gtk-4.0").exists() {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// List icon themes (index.theme present, not cursor-only).
pub fn list_icon_themes() -> Vec<String> {
    let mut names = Vec::new();
    for dir in icon_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || names.contains(&name) {
                continue;
            }
            // An icon theme has index.theme and at least one icon dir/file
            // other than "cursors".
            if e.path().join("index.theme").exists()
                && !e.path().join("cursors").join("index.theme").exists()
            {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// List cursor themes (a "cursors" subdir with real cursor files).
pub fn list_cursor_themes() -> Vec<String> {
    let mut names = Vec::new();
    for dir in icon_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || names.contains(&name) {
                continue;
            }
            if e.path().join("cursors").is_dir() {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Install a theme archive (.tar, .tar.gz, .tar.xz, .zip handled by bsdtar)
/// into the user theme dir for `kind` ("themes" or "icons").
/// Returns an error string on failure.
pub fn install_archive(archive: &std::path::Path, kind: &str) -> Result<(), String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME not set")?;
    let target = home.join(".local/share").join(kind);
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;

    let out = Command::new("tar")
        .args([
            "-xf",
            &archive.to_string_lossy(),
            "-C",
            &target.to_string_lossy(),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
