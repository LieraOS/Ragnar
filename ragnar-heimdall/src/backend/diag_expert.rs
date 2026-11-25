// Copyright (C) Jörg Schröder
//! Expert diagnostics, part 1 — checks that go beyond the quick
//! battery: captive-portal detection, IPv6 connectivity, path/MRU
//! probing and time synchronisation. Everything stays read-only;
//! a missing tool degrades the check to Skip, never to failure.

use std::process::Command;
use std::time::Instant;

use super::diag::{Check, Status};

/// Canonical page served by Firefox/NSPR captive-portal probes — a
/// correct network returns the literal "success\n" body; anything
/// else (redirect, login page) means a portal is intercepting.
const PORTAL_URL: &str = "http://detectportal.firefox.com/canonical.html";

/// Host used for external reachability probes.
const PROBE_HOST: &str = "1.1.1.1";

/// Captive-portal detection: expected-body comparison instead of
/// "did HTTP answer" — a portal answers 200 with a login page.
pub fn suite_portal() -> Vec<Check> {
    if !super::system::binary_exists("curl") {
        return vec![Check::new(
            "Captive portal",
            "portal probe",
            Status::Skip,
            "curl not installed",
        )];
    }
    let out = Command::new("curl")
        .args(["-s", "-m", "5", "-o", "-", "-w", "%{http_code}", PORTAL_URL])
        .output();
    let Some(out) = out.ok() else {
        return vec![Check::new(
            "Captive portal",
            "portal probe",
            Status::Skip,
            "probe request failed",
        )];
    };
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    // `-w %{http_code}` appends the 3-digit code after the body.
    let code = stdout.trim_end().chars().rev().take(3).collect::<String>();
    let code: String = code.chars().rev().collect();
    let body = &stdout[..stdout.trim_end().len().saturating_sub(3)];
    let check = if code == "200" && body.trim() == "success" {
        Check::new(
            "Captive portal",
            "portal check",
            Status::Pass,
            "expected response — no portal intercepting",
        )
    } else if out.status.success() {
        Check::new(
            "Captive portal",
            "portal check",
            Status::Warn,
            format!("unexpected reply (HTTP {code}) — captive portal likely"),
        )
    } else {
        Check::new(
            "Captive portal",
            "portal check",
            Status::Skip,
            "no HTTP reply — offline or filtered",
        )
    };
    vec![check]
}

/// IPv6 end-to-end: default v6 route present, v6 ping to gateway
/// and to an external anycast resolver.
pub fn suite_ipv6() -> Vec<Check> {
    let mut out = Vec::new();
    let route6 = Command::new("ip")
        .args(["-6", "-j", "route", "show", "default"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout).ok())
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    out.push(if route6 {
        Check::new(
            "IPv6",
            "default route",
            Status::Pass,
            "IPv6 default route present",
        )
    } else {
        Check::new(
            "IPv6",
            "default route",
            Status::Warn,
            "no IPv6 default route — IPv6 may be off or the network is v4-only",
        )
    });
    if route6 {
        let ok = Command::new("ping")
            .args(["-6", "-c", "2", "-W", "2", "2606:4700:4700::1111"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        out.push(if ok {
            Check::new(
                "IPv6",
                "external reachability",
                Status::Pass,
                "v6 ping to Cloudflare OK",
            )
        } else {
            Check::new(
                "IPv6",
                "external reachability",
                Status::Fail,
                "v6 route exists but external ping fails",
            )
        });
    }
    out
}

/// Path analysis: `tracepath` hop list plus a DF-bit MTU probe
/// against the probe host — the largest payload that passes
/// pinpoints the effective path MTU.
pub fn suite_path() -> Vec<Check> {
    let mut out = Vec::new();
    if super::system::binary_exists("tracepath") {
        let t = Instant::now();
        let res = Command::new("tracepath")
            .args(["-n", "-m", "8", PROBE_HOST])
            .output();
        match res {
            Ok(o) if o.status.success() => {
                let hops = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count();
                out.push(Check::new(
                    "Path analysis",
                    "tracepath",
                    Status::Pass,
                    format!("{hops} lines in {:.1}s", t.elapsed().as_secs_f32()),
                ));
            }
            _ => out.push(Check::new(
                "Path analysis",
                "tracepath",
                Status::Warn,
                "trace failed or timed out",
            )),
        }
    } else {
        out.push(Check::new(
            "Path analysis",
            "tracepath",
            Status::Skip,
            "iputils-tracepath not installed",
        ));
    }

    // DF-bit MTU probe: binary search on ICMP payload size.
    if super::system::binary_exists("ping") {
        let mut low = 1280u32;
        let mut high = 1472u32;
        let mut best = 0u32;
        for _ in 0..6 {
            let mid = (low + high) / 2;
            let ok = Command::new("ping")
                .args([
                    "-c",
                    "1",
                    "-W",
                    "1",
                    "-M",
                    "do",
                    "-s",
                    &mid.to_string(),
                    PROBE_HOST,
                ])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                best = mid;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
            if low > high {
                break;
            }
        }
        out.push(if best > 0 {
            Check::new(
                "Path analysis",
                "path MTU",
                Status::Pass,
                format!("payload {best} B → MTU ≈ {}", best + 28),
            )
        } else {
            Check::new(
                "Path analysis",
                "path MTU",
                Status::Warn,
                "no DF probe passed — path may drop ICMP",
            )
        });
    }
    out
}

/// NTP/time sync — an unsynchronised clock breaks TLS, Kerberos
/// and every certificate check downstream.
pub fn suite_timesync() -> Vec<Check> {
    if !super::system::binary_exists("timedatectl") {
        return vec![Check::new(
            "Time sync",
            "NTP",
            Status::Skip,
            "timedatectl not installed",
        )];
    }
    let out = Command::new("timedatectl")
        .args(["show", "-p", "NTPSynchronized", "--value"])
        .output();
    match out {
        Ok(o) => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            vec![if v == "yes" {
                Check::new("Time sync", "NTP", Status::Pass, "clock synchronised")
            } else {
                Check::new(
                    "Time sync",
                    "NTP",
                    Status::Warn,
                    "clock not synchronised — TLS and kerberos can fail",
                )
            }]
        }
        Err(_) => vec![Check::new(
            "Time sync",
            "NTP",
            Status::Skip,
            "timedatectl failed",
        )],
    }
}
