// Copyright (C) Jörg Schröder
//! `vpn` verb — status/import/up/down/delete/autostart for
//! WireGuard and OpenVPN profiles. Mirrors the shell helper's
//! behaviour: strict name validation, staged /tmp imports only,
//! unit names derived from the kind, never user text.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use super::exec::{reject, run as exec_run, run_quiet, VerbResult};
use super::validate;

/// Config path per kind — the two directories are fixed constants.
fn conf_path(kind: &str, name: &str) -> Option<String> {
    match kind {
        "wg" => Some(format!("/etc/wireguard/{name}.conf")),
        "ovpn" => Some(format!("/etc/openvpn/client/{name}.conf")),
        _ => None,
    }
}

/// systemd unit name per kind — templated units only.
fn unit_name(kind: &str, name: &str) -> Option<String> {
    match kind {
        "wg" => Some(format!("wg-quick@{name}")),
        "ovpn" => Some(format!("openvpn-client@{name}")),
        _ => None,
    }
}

fn kind_ok(kind: &str) -> bool {
    matches!(kind, "wg" | "ovpn")
}

/// `systemctl is-enabled` answer as plain text.
fn enabled(unit: &str) -> String {
    Command::new("systemctl")
        .args(["is-enabled", unit])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

/// `vpn status` — machine-readable `kind|name|state|autostart|rx|tx`
/// per profile, same format the shell helper printed.
fn status() -> VerbResult {
    if let Ok(rd) = fs::read_dir("/etc/wireguard") {
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "conf") != Some(true) {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let mut state = "down";
            let mut rx = String::new();
            let mut tx = String::new();
            let up = Command::new("wg")
                .args(["show", "interfaces"])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .split_whitespace()
                        .any(|i| i == name)
                })
                .unwrap_or(false);
            if up {
                state = "up";
                if let Ok(o) = Command::new("wg").args(["show", name, "transfer"]).output() {
                    if let Some(line) = String::from_utf8_lossy(&o.stdout).lines().last() {
                        let f: Vec<&str> = line.split_whitespace().collect();
                        rx = f.get(1).unwrap_or(&"").to_string();
                        tx = f.get(2).unwrap_or(&"").to_string();
                    }
                }
            }
            println!("wg|{name}|{state}|{}|{rx}|{tx}", enabled(&format!("wg-quick@{name}")));
        }
    }
    if let Ok(rd) = fs::read_dir("/etc/openvpn/client") {
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "conf") != Some(true) {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let active = Command::new("systemctl")
                .args(["is-active", &format!("openvpn-client@{name}")])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false);
            let state = if active { "up" } else { "down" };
            println!(
                "ovpn|{name}|{state}|{}||",
                enabled(&format!("openvpn-client@{name}"))
            );
        }
    }
    Ok(0)
}

/// `vpn import wg|ovpn <name> <staged-file>` — only files staged
/// under /tmp/ragnar-heimdall-vpn-* are accepted, installed 0600
/// root:root into the kind's config dir, source removed.
fn import(args: &[String]) -> VerbResult {
    let (Some(kind), Some(name), Some(src)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) else {
        return reject("vpn import: missing arguments");
    };
    if !kind_ok(kind) || !validate::profile_name(name) {
        return reject("bad kind or name");
    }
    let Some(dst) = conf_path(kind, name) else {
        return reject("bad kind");
    };
    if !validate::staged_path(src) || !std::path::Path::new(src).is_file() {
        return reject("refused source");
    }
    if let Some(parent) = std::path::Path::new(&dst).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::copy(src, &dst).is_err() {
        return reject("import copy failed");
    }
    let _ = fs::set_permissions(&dst, fs::Permissions::from_mode(0o600));
    let _ = fs::remove_file(src);
    Ok(0)
}

/// `vpn up|down wg|ovpn <name>` — wg-quick for wireguard, the
/// templated openvpn-client unit for ovpn.
fn up_down(action: &str, args: &[String]) -> VerbResult {
    let (Some(kind), Some(name)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("vpn up/down: missing arguments");
    };
    if !kind_ok(kind) || !validate::profile_name(name) {
        return reject("bad kind or name");
    }
    let Some(dst) = conf_path(kind, name) else {
        return reject("bad kind");
    };
    if !std::path::Path::new(&dst).is_file() {
        return reject("no such profile");
    }
    if kind == "wg" {
        exec_run("wg-quick", &[action, name])
    } else {
        exec_run("systemctl", &[if action == "up" { "start" } else { "stop" }, &format!("openvpn-client@{name}")])
    }
}

/// `vpn delete wg|ovpn <name>` — down first, then remove config
/// and any autostart.
fn delete(args: &[String]) -> VerbResult {
    let (Some(kind), Some(name)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("vpn delete: missing arguments");
    };
    if !kind_ok(kind) || !validate::profile_name(name) {
        return reject("bad kind or name");
    }
    let Some(dst) = conf_path(kind, name) else {
        return reject("bad kind");
    };
    let Some(unit) = unit_name(kind, name) else {
        return reject("bad kind");
    };
    if kind == "wg" {
        run_quiet("wg-quick", &["down", name]);
    } else {
        run_quiet("systemctl", &["stop", &unit]);
    }
    run_quiet("systemctl", &["disable", &unit]);
    let _ = fs::remove_file(&dst);
    Ok(0)
}

/// `vpn autostart wg|ovpn <name> on|off`.
fn autostart(args: &[String]) -> VerbResult {
    let (Some(kind), Some(name), Some(mode)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) else {
        return reject("vpn autostart: missing arguments");
    };
    if !kind_ok(kind) || !validate::profile_name(name) {
        return reject("bad kind or name");
    }
    let Some(unit) = unit_name(kind, name) else {
        return reject("bad kind");
    };
    match mode {
        "on" => exec_run("systemctl", &["enable", &unit]),
        "off" => exec_run("systemctl", &["disable", &unit]),
        other => reject(&format!("autostart '{other}' invalid")),
    }
}

/// `vpn <status|import|up|down|delete|autostart>`.
pub fn run(args: &[String]) -> VerbResult {
    let Some(action) = args.first().map(String::as_str) else {
        return reject("vpn: no action");
    };
    let rest = &args[1..];
    match action {
        "status" => status(),
        "import" => import(rest),
        "up" | "down" => up_down(action, rest),
        "delete" => delete(rest),
        "autostart" => autostart(rest),
        other => reject(&format!("vpn '{other}' not allowed")),
    }
}
