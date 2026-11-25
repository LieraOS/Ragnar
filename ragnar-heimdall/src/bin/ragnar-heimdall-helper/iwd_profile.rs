// Copyright (C) Jörg Schröder
//! `iwd-profile` verb — read/write/list of iwd network profiles in
//! /var/lib/iwd. Filenames are validated to a single component with
//! a known suffix; writes go through a same-dir temp file + rename
//! so iwd never sees a half-written profile.

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;

use super::exec::{reject, VerbResult};
use super::validate;

const IWD_DIR: &str = "/var/lib/iwd";

/// `iwd-profile list|read|write <file>` — stdin → profile on write.
pub fn run(args: &[String]) -> VerbResult {
    let Some(action) = args.first().map(String::as_str) else {
        return reject("iwd-profile: no action");
    };
    match action {
        "list" => {
            match fs::read_dir(IWD_DIR) {
                Ok(rd) => {
                    for e in rd.flatten() {
                        if let Some(name) = e.file_name().to_str() {
                            println!("{name}");
                        }
                    }
                    Ok(0)
                }
                Err(_) => Ok(0),
            }
        }
        "read" => {
            let name = args.get(1).map(String::as_str).unwrap_or("");
            if !validate::iwd_file(name) {
                return reject("bad profile name");
            }
            match fs::read_to_string(format!("{IWD_DIR}/{name}")) {
                Ok(text) => {
                    print!("{text}");
                    Ok(0)
                }
                Err(e) => {
                    eprintln!("ragnar-heimdall-helper: read failed: {e}");
                    Err(())
                }
            }
        }
        "write" => {
            let name = args.get(1).map(String::as_str).unwrap_or("");
            if !validate::iwd_file(name) {
                return reject("bad profile name");
            }
            let mut buf = String::new();
            if std::io::stdin().read_to_string(&mut buf).is_err() {
                return reject("could not read stdin");
            }
            let tmp = format!("{IWD_DIR}/{name}.tmp.{}", std::process::id());
            let dst = format!("{IWD_DIR}/{name}");
            if fs::write(&tmp, buf).is_err() {
                return reject("profile write failed");
            }
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
            match fs::rename(&tmp, &dst) {
                Ok(()) => Ok(0),
                Err(e) => {
                    let _ = fs::remove_file(&tmp);
                    eprintln!("ragnar-heimdall-helper: rename failed: {e}");
                    Err(())
                }
            }
        }
        other => reject(&format!("iwd-profile {other} not allowed")),
    }
}
