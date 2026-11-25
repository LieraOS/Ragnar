// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — configuration backup & restore
// Copyright (C) 2026 LieraOS Project
//
// Exports every configuration source Ragnar Settings can write into
// a single .tar.gz the user picks via file dialog, and restores it
// the same way. The manifest below is THE list: whenever a panel
// starts writing a new file or schema, it is registered here once —
// that is the only maintenance this feature needs.

mod staging;

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita::prelude::{AdwDialogExt, AlertDialogExt};
use staging::{build_sys_script, copy_entry, staging_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

/// User-level files/dirs relative to `~/.config` (the directory
/// g_get_user_config_dir() returns). Trailing `/` marks a directory
/// copied recursively.
const USER_PATHS: &[&str] = &[
    "ragnar-settings/settings.ini",      // advanced mode flag
    "ragnar-settings/sounds.ini",        // event sounds + volumes
    "ragnar-settings/wallpaper-outputs", // per-monitor wallpaper map
    "ragnar-raven/settings.ini",         // notification hub prefs
    "ragnar-raven/sounds/",              // per-appointment alert copies
    "ragnar-osd/settings.ini",           // OSD prefs
    "ragnar-keyring/settings.ini",       // keyring unlock policy
    "ragnar-desktop/labwc/",             // rc.xml, environment, autostart
    "gammastep/config.ini",              // night light
    "autostart/",                        // user autostart overrides
    "gtk-4.0/gtk.css",                   // custom accent override
    "mimeapps.list",                     // default applications
];

/// System files written via pkexec — restoring them needs the same
/// privilege. Absent or unreadable entries are skipped silently.
const SYSTEM_PATHS: &[&str] = &[
    "/etc/apt/apt.conf.d/52ragnar-auto-upgrades",
    "/etc/lightdm/lightdm.conf.d/50-ragnar-autologin.conf",
    "/etc/locale.conf", // language panel (via localectl)
    "/etc/default/locale",
    "/etc/default/keyboard", // console keyboard layout
    "/etc/localtime",        // symlink → zoneinfo (datetime panel)
];

/// gsettings schemas dumped whole. Whole-schema snapshots capture
/// every key — including ones added to panels later — so new keys
/// in an already-covered schema never silently skip the backup.
const GSETTINGS_SCHEMAS: &[&str] = &[
    "org.gnome.desktop.interface",
    "org.gnome.desktop.a11y.interface",
    "org.gnome.desktop.background",
    "org.gnome.desktop.screensaver",
    "org.gnome.desktop.session",
    "org.gnome.settings-daemon.plugins.color",
    "org.gnome.settings-daemon.plugins.power",
];

/// Every schema under this prefix is dumped too — new Ragnar
/// schemas are captured automatically, no manifest edit needed.
const GSETTINGS_PREFIX: &str = "org.lieraos.ragnar";

/// Top directory name inside the archive.
const ARCHIVE_ROOT: &str = "ragnar-config";
/// Format version — bump when the archive layout changes.
const FORMAT_VERSION: &str = "1";

// -------------------------------------------------------------------
// Public entry points (called from the hub footer)
// -------------------------------------------------------------------

/// "Back up configuration…" — file-save dialog, then export.
pub fn export_dialog(parent: &gtk4::Window) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(&gettext("Save configuration backup"));
    dialog.set_filters(Some(&tgz_filters()));
    dialog.set_initial_name(Some(&format!("ragnar-config-{}.tar.gz", stamp_now())));
    let win = parent.clone();
    dialog.save(Some(parent), None::<&gtk4::gio::Cancellable>, move |res| {
        let Ok(file) = res else { return };
        let Some(path) = file.path() else { return };
        let report = export(&path);
        finish_dialog(&win, &report, true);
    });
}

/// "Restore configuration…" — file-open dialog, then import.
pub fn restore_dialog(parent: &gtk4::Window) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title(&gettext("Choose a configuration backup"));
    dialog.set_filters(Some(&tgz_filters()));
    let win = parent.clone();
    dialog.open(Some(parent), None::<&gtk4::gio::Cancellable>, move |res| {
        let Ok(file) = res else { return };
        let Some(path) = file.path() else { return };
        let report = restore(&path);
        finish_dialog(&win, &report, false);
    });
}

// -------------------------------------------------------------------
// Export
// -------------------------------------------------------------------

/// Stage all manifest sources into a temp dir and tar them into
/// `out`. Returns a human-readable report for the result dialog.
fn export(out: &Path) -> String {
    let staging = staging_dir("backup");
    let root = staging.join(ARCHIVE_ROOT);
    let mut files = 0u32;

    for entry in USER_PATHS {
        let rel = entry.trim_end_matches('/');
        let src = config_dir().join(rel);
        if copy_entry(&src, &root.join("config").join(rel), src.is_dir()) {
            files += 1;
        }
    }
    for entry in SYSTEM_PATHS {
        let src = Path::new(entry);
        // /etc/x → staged etc/x (the etc/ tree mirrors the real /etc).
        let rel = src.strip_prefix("/etc").unwrap_or(src);
        if copy_entry(src, &root.join("etc").join(rel), src.is_dir()) {
            files += 1;
        }
    }

    // gsettings: fixed schemas + everything under the ragnar prefix.
    let gdir = root.join("gsettings");
    let _ = std::fs::create_dir_all(&gdir);
    for schema in all_schemas() {
        if let Some(dump) = dump_schema(&schema) {
            if std::fs::write(gdir.join(&schema), dump).is_ok() {
                files += 1;
            }
        }
    }

    let _ = std::fs::write(
        root.join("MANIFEST"),
        format!(
            "format={}\ncreated={}\napp=ragnar-settings {}\nfiles={}\n",
            FORMAT_VERSION,
            stamp_now(),
            env!("CARGO_PKG_VERSION"),
            files
        ),
    );

    let ok = Command::new("tar")
        .arg("-czf")
        .arg(out)
        .arg("-C")
        .arg(&staging)
        .arg(ARCHIVE_ROOT)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = std::fs::remove_dir_all(&staging);
    if ok {
        format!(
            "{}\n\n{} {}",
            gettext("Backup written."),
            files,
            gettext("entries saved")
        )
    } else {
        gettext("Backup failed — could not write the archive.").to_string()
    }
}

// -------------------------------------------------------------------
// Restore
// -------------------------------------------------------------------

/// Extract the archive, copy files back, replay gsettings, and
/// install system files via pkexec.
fn restore(archive: &Path) -> String {
    let staging = staging_dir("restore");
    let ok = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let root = staging.join(ARCHIVE_ROOT);
    if !ok || !root.join("MANIFEST").exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return gettext("Not a valid Ragnar configuration backup.").to_string();
    }

    let mut restored = 0u32;
    // Merge each staged ~/.config entry back over the live one.
    if let Ok(rd) = std::fs::read_dir(root.join("config")) {
        for e in rd.flatten() {
            let dst = config_dir().join(e.file_name());
            if copy_entry(&e.path(), &dst, e.path().is_dir()) {
                restored += 1;
            }
        }
    }

    // gsettings replay: "KEY VALUE" per line per schema file.
    if let Ok(rd) = std::fs::read_dir(root.join("gsettings")) {
        for e in rd.flatten() {
            let Some(schema) = e.file_name().to_str().map(String::from) else {
                continue;
            };
            let Ok(body) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            for line in body.lines() {
                if let Some((key, value)) = line.split_once(' ') {
                    let _ = Command::new("gsettings")
                        .args(["set", &schema, key, value])
                        .status();
                    restored += 1;
                }
            }
        }
    }

    // System files: one pkexec script copies everything root-owned.
    let etc = root.join("etc");
    let mut sys_note = String::new();
    if let Some(script) = build_sys_script(&etc) {
        let sp = staging.join("restore-sys.sh");
        if std::fs::write(&sp, &script).is_ok() {
            let r = Command::new("pkexec")
                .args(["sh", &sp.to_string_lossy()])
                .status();
            sys_note = match r {
                Ok(s) if s.success() => gettext("System settings restored.").to_string(),
                _ => gettext("System settings skipped (authentication cancelled or failed).")
                    .to_string(),
            };
        }
    }

    let _ = std::fs::remove_dir_all(&staging);
    let mut msg = format!(
        "{}\n\n{} {}",
        gettext("Restore finished."),
        restored,
        gettext("entries applied")
    );
    if !sys_note.is_empty() {
        msg.push_str(&format!("\n{sys_note}"));
    }
    msg.push_str(&format!(
        "\n{}",
        gettext("Log out and back in for all changes to take effect.")
    ));
    msg
}

// -------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------

/// ~/.config — same base the panels resolve against.
fn config_dir() -> PathBuf {
    gtk4::glib::user_config_dir()
}

/// `gsettings list-recursively` reduced to "KEY VALUE" lines — the
/// schema column is redundant (the file is already named after it).
fn dump_schema(schema: &str) -> Option<String> {
    let o = Command::new("gsettings")
        .args(["list-recursively", schema])
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&o.stdout);
    if body.trim().is_empty() {
        return None;
    }
    Some(
        body.lines()
            .filter_map(|l| l.strip_prefix(&format!("{schema} ")))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// All schemas to dump: the fixed list plus every schema under the
/// Ragnar prefix that exists on this system.
fn all_schemas() -> Vec<String> {
    let mut v: Vec<String> = GSETTINGS_SCHEMAS.iter().map(|s| s.to_string()).collect();
    if let Ok(o) = Command::new("gsettings").arg("list-schemas").output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if line.starts_with(GSETTINGS_PREFIX) && !v.iter().any(|s| s == line) {
                v.push(line.to_string());
            }
        }
    }
    v
}

/// Timestamp for archive names and the manifest.
fn stamp_now() -> String {
    gtk4::glib::DateTime::now_local()
        .ok()
        .and_then(|d| d.format("%Y-%m-%d_%H-%M").ok())
        .map(|g| g.to_string())
        .unwrap_or_default()
}

/// File filter accepting .tar.gz / .tgz archives.
fn tgz_filters() -> gtk4::gio::ListStore {
    let f = gtk4::FileFilter::new();
    f.set_name(Some("*.tar.gz"));
    f.add_suffix("gz");
    let fs = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    fs.append(&f);
    fs
}

/// Modal result dialog after export/restore.
fn finish_dialog(win: &gtk4::Window, msg: &str, was_export: bool) {
    let d = libadwaita::AlertDialog::builder()
        .heading(if was_export {
            gettext("Backup")
        } else {
            gettext("Restore")
        })
        .body(msg)
        .build();
    d.add_response("ok", &gettext("OK"));
    d.present(Some(win));
}
