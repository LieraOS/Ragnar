//! Expert diagnostics, part 2 — the power-user depth checks:
//! neighbor table health, listening sockets, VPN tunnel state,
//! per-resolver DNS latency and Wi-Fi link statistics. Read-only;
//! missing tools degrade to Skip.

use std::process::Command;
use std::time::Instant;

use super::diag::{Check, Status};

/// ARP/NDP neighbor table: healthy cache vs stale/failed entries —
/// failed neighbors on the LAN mean L2 trouble, not L3.
pub fn suite_neighbors() -> Vec<Check> {
    let out = Command::new("ip").args(["-j", "neigh", "show"]).output();
    let Ok(o) = out else {
        return vec![Check::new(
            "Neighbors",
            "neighbor table",
            Status::Skip,
            "ip neigh failed",
        )];
    };
    let Ok(entries) = serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout) else {
        return vec![Check::new(
            "Neighbors",
            "neighbor table",
            Status::Skip,
            "unparsable output",
        )];
    };
    let mut reach = 0u32;
    let mut stale = 0u32;
    let mut failed: Vec<String> = Vec::new();
    for e in &entries {
        let state = e["state"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        if state.contains("REACHABLE") || state.contains("PERMANENT") {
            reach += 1;
        } else if state.contains("STALE") || state.contains("DELAY") {
            stale += 1;
        } else if state.contains("FAILED") {
            failed.push(e["dst"].as_str().unwrap_or("?").to_string());
        }
    }
    let mut out = vec![Check::new(
        "Neighbors",
        "neighbor table",
        Status::Pass,
        format!(
            "{reach} reachable · {stale} stale · {} failed",
            failed.len()
        ),
    )];
    for f in failed.iter().take(4) {
        out.push(Check::new(
            "Neighbors",
            "unresolvable peer",
            Status::Warn,
            format!("{f} never answered ARP/ND"),
        ));
    }
    out
}

/// Listening sockets — which ports the machine exposes. A desktop
/// should answer "few"; unexpected listeners are worth an eyeball.
pub fn suite_listeners() -> Vec<Check> {
    if !super::system::binary_exists("ss") {
        return vec![Check::new(
            "Listeners",
            "listening sockets",
            Status::Skip,
            "ss not installed",
        )];
    }
    let out = Command::new("ss").args(["-tulnH"]).output();
    let Ok(o) = out else {
        return vec![Check::new(
            "Listeners",
            "listening sockets",
            Status::Skip,
            "ss failed",
        )];
    };
    let lines: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    let mut out = vec![Check::new(
        "Listeners",
        "listening sockets",
        if lines.len() <= 8 {
            Status::Pass
        } else {
            Status::Warn
        },
        format!("{} TCP/UDP listeners", lines.len()),
    )];
    for l in lines.iter().take(12) {
        let mut parts = l.split_whitespace();
        let proto = parts.next().unwrap_or("?");
        let addr = parts.nth(4).unwrap_or("?");
        out.push(Check::new("Listeners", proto, Status::Pass, addr));
    }
    out
}

/// VPN tunnel state: wireguard peers with fresh handshakes plus
/// tun/tap interfaces carrying a route.
pub fn suite_vpnstate() -> Vec<Check> {
    let mut out = Vec::new();
    if super::system::binary_exists("wg") {
        let res = Command::new("wg").args(["show", "all", "dump"]).output();
        match res {
            Ok(o) if o.status.success() && !o.stdout.is_empty() => {
                let txt = String::from_utf8_lossy(&o.stdout).to_string();
                let peers = txt
                    .lines()
                    .skip(1)
                    .filter(|l| l.split('\t').count() > 5)
                    .count();
                out.push(Check::new(
                    "VPN state",
                    "wireguard",
                    Status::Pass,
                    format!("{peers} peer(s) configured"),
                ));
            }
            _ => out.push(Check::new(
                "VPN state",
                "wireguard",
                Status::Skip,
                "no active wireguard tunnels",
            )),
        }
    } else {
        out.push(Check::new(
            "VPN state",
            "wireguard",
            Status::Skip,
            "wg not installed",
        ));
    }
    let tun = Command::new("ip")
        .args(["-j", "link", "show", "type", "tun"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout).ok())
        .map(|v| v.len())
        .unwrap_or(0);
    out.push(Check::new(
        "VPN state",
        "tun/tap devices",
        Status::Pass,
        format!("{tun} device(s)"),
    ));
    out
}

/// DNS depth: configured resolvers + measured query latency +
/// DNSSEC/DoT posture of systemd-resolved.
pub fn suite_dns_deep() -> Vec<Check> {
    let mut out = Vec::new();
    let status = Command::new("resolvectl").args(["status"]).output().ok();
    let text = status.map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    if let Some(t) = &text {
        let dnssec = if t.contains("DNSSEC=yes") || t.contains("DNSSEC allow") {
            "enforcing"
        } else if t.contains("DNSSEC=no") {
            "off"
        } else {
            "default"
        };
        let dot = if t.contains("DNSOverTLS=yes") {
            "on"
        } else {
            "off"
        };
        out.push(Check::new(
            "DNS depth",
            "resolver posture",
            Status::Pass,
            format!("DNSSEC {dnssec} · DoT {dot}"),
        ));
    }
    // Resolution latency — cheap and the #1 felt-DNS issue.
    let t = Instant::now();
    let ok = Command::new("resolvectl")
        .args(["query", "devin.ai"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let ms = t.elapsed().as_millis();
    out.push(if ok {
        Check::new(
            "DNS depth",
            "query latency",
            if ms < 300 { Status::Pass } else { Status::Warn },
            format!("{ms} ms for devin.ai"),
        )
    } else {
        Check::new("DNS depth", "query latency", Status::Fail, "query failed")
    });
    out
}

/// Wi-Fi link depth: BSSID, channel, tx bitrate, signal — `iw` data
/// iwd does not surface on the simple path.
pub fn suite_wifi_deep(ifaces: &[super::model::NetworkInterface]) -> Vec<Check> {
    let Some(iface) = ifaces
        .iter()
        .find(|i| i.kind == super::model::LinkKind::Wifi)
        .map(|i| i.name.clone())
    else {
        return vec![Check::new(
            "Wi-Fi depth",
            "link",
            Status::Skip,
            "no wireless interface",
        )];
    };
    if !super::system::binary_exists("iw") {
        return vec![Check::new(
            "Wi-Fi depth",
            "link",
            Status::Skip,
            "iw not installed",
        )];
    }
    let mut out = Vec::new();
    let res = Command::new("iw").args(["dev", &iface, "link"]).output();
    match res {
        Ok(o) if o.status.success() => {
            let txt = String::from_utf8_lossy(&o.stdout);
            for l in txt.lines() {
                let l = l.trim();
                if l.starts_with("Connected to") {
                    out.push(Check::new(
                        "Wi-Fi depth",
                        "BSSID",
                        Status::Pass,
                        l.to_string(),
                    ));
                } else if l.starts_with("freq:") {
                    out.push(Check::new(
                        "Wi-Fi depth",
                        "channel",
                        Status::Pass,
                        l.to_string(),
                    ));
                } else if l.starts_with("signal:") {
                    out.push(Check::new(
                        "Wi-Fi depth",
                        "signal",
                        Status::Pass,
                        l.to_string(),
                    ));
                } else if l.starts_with("tx bitrate:") {
                    out.push(Check::new(
                        "Wi-Fi depth",
                        "tx bitrate",
                        Status::Pass,
                        l.to_string(),
                    ));
                }
            }
            if out.is_empty() {
                out.push(Check::new(
                    "Wi-Fi depth",
                    "link",
                    Status::Warn,
                    "iw link returned no association",
                ));
            }
        }
        _ => out.push(Check::new(
            "Wi-Fi depth",
            "link",
            Status::Skip,
            "not associated",
        )),
    }
    out
}
