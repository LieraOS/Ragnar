// Copyright (C) Jörg Schröder
//! Compile gettext catalogs (po/*.po) into target/locale/<lang>/
//! LC_MESSAGES/ragnar-settings.mo before cargo-deb assembles the
//! package — the .mo paths in `[package.metadata.deb] assets` point
//! at that directory. Skips quietly when msgfmt is missing so the
//! build never hard-fails on a development machine without
//! gettext tooling installed.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=po");
    let Ok(entries) = std::fs::read_dir("po") else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("po") {
            continue;
        }
        let Some(lang) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let dir = Path::new("target")
            .join("locale")
            .join(lang)
            .join("LC_MESSAGES");
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let out = dir.join("ragnar-settings.mo");
        // msgfmt validates while compiling — a broken catalog fails
        // the build here, not silently at runtime.
        let status = Command::new("msgfmt")
            .arg("--check")
            .arg(&path)
            .arg("-o")
            .arg(&out)
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(_) => panic!("msgfmt failed for {}", path.display()),
            Err(_) => {
                println!(
                    "cargo:warning=msgfmt not found, skipping {}",
                    path.display()
                )
            }
        }
    }
}
