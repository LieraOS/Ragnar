//! File-writing verbs — networkd/sysctl.d/main.conf mutations.
//! Every path is composed from validated parts only; writes are
//! complete-file replacements with explicit permissions, never
//! in-place edits of arbitrary lines.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::exec::{reject, run, run_quiet, VerbResult};
use super::validate;

/// `sysctl <iface> <0|1>` — only the IPv6 disable toggle. Applied
/// live via `sysctl -w` AND persisted to sysctl.d so the choice
/// survives reboots.
pub fn sysctl(args: &[String]) -> VerbResult {
    let (Some(iface), Some(val)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("sysctl: missing arguments");
    };
    if !validate::iface(iface) {
        return reject("bad iface");
    }
    if !matches!(val, "0" | "1") {
        return reject("sysctl: only 0|1 allowed");
    }
    let _ = fs::create_dir_all("/etc/sysctl.d");
    let path = format!("/etc/sysctl.d/90-ragnar-heimdall-{iface}.conf");
    if fs::write(&path, format!("net.ipv6.conf.{iface}.disable_ipv6={val}\n")).is_err() {
        return reject("sysctl: could not write sysctl.d file");
    }
    run(
        "sysctl",
        &["-w", &format!("net.ipv6.conf.{iface}.disable_ipv6={val}")],
    )
}

/// `metric <iface> <metric|auto>` — DHCP route preference via a
/// networkd drop-in on our per-interface .network file. When no
/// managed file exists a DHCP baseline is created so the interface
/// keeps working under our 10- priority. `auto` writes an empty
/// RouteMetric — the documented reset semantic.
pub fn metric(args: &[String]) -> VerbResult {
    let (Some(iface), Some(metric)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("metric: missing arguments");
    };
    if !validate::iface(iface) {
        return reject("bad iface");
    }
    if !validate::metric(metric) {
        return reject("bad metric");
    }
    let val = if metric == "auto" { "" } else { metric };
    let conf = format!("/etc/systemd/network/10-ragnar-heimdall-{iface}.network");
    let dropdir = format!("{conf}.d");
    if fs::create_dir_all(&dropdir).is_err() {
        return reject("metric: could not create drop-in dir");
    }
    if !std::path::Path::new(&conf).exists() {
        let base = format!("[Match]\nName={iface}\n\n[Network]\nDHCP=yes\nIPv6AcceptRA=yes\n");
        if fs::write(&conf, base).is_err() {
            return reject("metric: could not write base network file");
        }
        let _ = fs::set_permissions(&conf, fs::Permissions::from_mode(0o644));
    }
    let drop = format!("{dropdir}/90-metric.conf");
    let body = format!("[DHCPv4]\nRouteMetric={val}\n\n[IPv6AcceptRA]\nRouteMetric={val}\n");
    if fs::write(&drop, body).is_err() {
        return reject("metric: could not write drop-in");
    }
    let _ = fs::set_permissions(&drop, fs::Permissions::from_mode(0o644));
    run_quiet("networkctl", &["reload"]);
    run_quiet("networkctl", &["reconfigure", iface]);
    Ok(0)
}

/// `networkd <iface> dhcp | static <cidr> <gw>` — per-interface
/// .network file at 10- priority so it sorts BEFORE the global
/// 50- catch-all (`en* wl*`) and actually applies.
pub fn networkd(args: &[String]) -> VerbResult {
    let (Some(iface), Some(mode)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("networkd: missing arguments");
    };
    if !validate::iface(iface) {
        return reject("bad iface");
    }
    let _ = fs::create_dir_all("/etc/systemd/network");
    let conf = format!("/etc/systemd/network/10-ragnar-heimdall-{iface}.network");
    // Stale 60- files never won against the 50- catch-all — clean up.
    let _ = fs::remove_file(format!("/etc/systemd/network/60-ragnar-heimdall-{iface}.network"));
    let body = match mode {
        "dhcp" => format!("[Match]\nName={iface}\n\n[Network]\nDHCP=yes\nIPv6AcceptRA=yes\n"),
        "static" => {
            let (Some(cidr), Some(gw)) = (
                args.get(2).map(String::as_str),
                args.get(3).map(String::as_str),
            ) else {
                return reject("networkd static: missing cidr/gateway");
            };
            if !validate::cidr(cidr) {
                return reject("bad cidr");
            }
            if !validate::ipv4(gw) {
                return reject("bad gateway");
            }
            format!(
                "[Match]\nName={iface}\n\n[Network]\nDHCP=no\nIPv6AcceptRA=yes\nAddress={cidr}\nGateway={gw}\n"
            )
        }
        other => return reject(&format!("networkd {other} not allowed")),
    };
    if fs::write(&conf, body).is_err() {
        return reject("networkd: could not write network file");
    }
    let _ = fs::set_permissions(&conf, fs::Permissions::from_mode(0o644));
    match run("networkctl", &["reload"]) {
        Ok(0) => Ok(0),
        _ => run("systemctl", &["restart", "systemd-networkd"]),
    }
}

/// `iwd set <key> <value>` — whitelisted [General] keys of
/// /etc/iwd/main.conf only. Rewrites the single key line, keeps
/// the rest of the file untouched, then restarts iwd.
pub fn iwd(args: &[String]) -> VerbResult {
    let (Some(sub), Some(key), Some(value)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) else {
        return reject("iwd: missing arguments");
    };
    if sub != "set" {
        return reject(&format!("iwd {sub} not allowed"));
    }
    const ALLOWED: &[(&str, &str)] = &[
        ("AddressRandomization", "disabled"),
        ("AddressRandomization", "once"),
        ("AddressRandomization", "network"),
        ("AddressRandomizationRange", "nic"),
        ("AddressRandomizationRange", "full"),
    ];
    if !ALLOWED.contains(&(key, value)) {
        return reject(&format!("iwd {key}={value} not allowed"));
    }
    let conf = "/etc/iwd/main.conf";
    let text = fs::read_to_string(conf).unwrap_or_default();
    let mut out = String::new();
    let mut replaced = false;
    let mut in_general = false;
    let mut inserted = false;
    for line in text.lines() {
        if line.trim() == "[General]" {
            in_general = true;
        } else if line.starts_with('[') {
            in_general = false;
        }
        if in_general && line.starts_with(&format!("{key}=")) {
            out.push_str(&format!("{key}={value}\n"));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
        if in_general && !replaced && !inserted && line.trim() == "[General]" {
            // Insert right after the section header if the key is absent.
            out.push_str(&format!("{key}={value}\n"));
            inserted = true;
        }
    }
    if !replaced && !inserted {
        out.push_str(&format!("\n[General]\n{key}={value}\n"));
    }
    if fs::write(conf, out).is_err() {
        return reject("iwd: could not write main.conf");
    }
    run("systemctl", &["restart", "iwd"])
}
