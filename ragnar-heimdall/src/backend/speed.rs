//! Internet performance checks: external IP, latency, download and
//! upload throughput against Cloudflare's public speedtest endpoints
//! via `curl` — a standard tool, no account, no API key. Everything
//! is read-only and unprivileged; a missing `curl` downgrades the
//! whole suite to `Skip` instead of failing.

use std::io::Write;
use std::process::{Command, Stdio};

use super::diag::{Check, Status};

/// Trace endpoint — returns the public IP plus PoP metadata as
/// `key=value` lines; doubles as the latency probe.
const TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
/// Download payload (~25 MiB) served by the speedtest worker.
const DOWN_URL: &str = "https://speed.cloudflare.com/__down?bytes=26214400";
/// Upload sink accepting a raw octet-stream POST.
const UP_URL: &str = "https://speed.cloudflare.com/__up";
/// Upload payload size (8 MiB) — large enough for a stable mean,
/// small enough to finish on slow uplinks.
const UP_BYTES: usize = 8 * 1024 * 1024;

/// Run every internet check; each returns its own `Check`.
pub fn run_all() -> Vec<Check> {
    if !super::system::binary_exists("curl") {
        return vec![Check::new(
            "Internet",
            "speedtest",
            Status::Skip,
            "curl not installed",
        )];
    }
    let mut out = Vec::new();
    let trace = fetch_trace();
    out.push(external_ip_check(trace.as_deref()));
    out.push(latency_check());
    out.push(download_check());
    out.push(upload_check());
    out
}

/// `curl` timing data for one GET: (exit_ok, ttfb_s, speed_Bps).
struct Timing {
    ttfb: f64,
    speed: f64,
}

/// `curl -w` format extracting connect latency and transfer speed.
const WRITE_OUT: &str = "%{time_starttransfer} %{speed_download} %{speed_upload}";

fn curl_timing(url: &str, extra: &[&str]) -> Option<Timing> {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", WRITE_OUT, "--max-time", "20"])
        .args(extra)
        .arg(url)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let ttfb = parts.next()?.parse().ok()?;
    let dl: f64 = parts.next()?.parse().ok()?;
    let ul: f64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    Some(Timing {
        ttfb,
        speed: if ul > 0.0 { ul } else { dl },
    })
}

/// Public IPv4/IPv6 from the trace endpoint's `ip=` field.
fn fetch_trace() -> Option<String> {
    let out = Command::new("curl")
        .args(["-s", "--max-time", "10", TRACE_URL])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("ip="))
        .map(str::to_string)
}

fn external_ip_check(ip: Option<&str>) -> Check {
    match ip {
        Some(ip) => Check::new("Internet", "external IP", Status::Pass, ip),
        None => Check::new(
            "Internet",
            "external IP",
            Status::Fail,
            "could not query cloudflare trace",
        ),
    }
}

/// TTFB of the trace endpoint ≈ internet RTT — works where ICMP is
/// firewalled and measures the same path the speedtest uses.
fn latency_check() -> Check {
    match curl_timing(TRACE_URL, &[]) {
        Some(t) => Check::new(
            "Internet",
            "latency",
            Status::Pass,
            format!("{:.0} ms to speedtest endpoint", t.ttfb * 1000.0),
        ),
        None => Check::new("Internet", "latency", Status::Fail, "endpoint unreachable"),
    }
}

fn download_check() -> Check {
    match curl_timing(DOWN_URL, &[]) {
        Some(t) if t.speed > 0.0 => Check::new(
            "Internet",
            "download",
            Status::Pass,
            format!("{:.1} Mbit/s", t.speed * 8.0 / 1_000_000.0),
        ),
        _ => Check::new(
            "Internet",
            "download",
            Status::Fail,
            "download test failed or timed out",
        ),
    }
}

fn upload_check() -> Check {
    // Stream the payload from stdin so no temp file is needed.
    let mut child = match Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            WRITE_OUT,
            "--max-time",
            "30",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/octet-stream",
            "--data-binary",
            "@-",
            UP_URL,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Check::new("Internet", "upload", Status::Fail, "could not start curl"),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let chunk = [0u8; 64 * 1024];
        for _ in 0..(UP_BYTES / chunk.len()) {
            if stdin.write_all(&chunk).is_err() {
                break;
            }
        }
    }
    let speed = child.wait_with_output().ok().and_then(|o| {
        if !o.status.success() {
            return None;
        }
        String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
    });
    if let Some(speed) = speed {
        Check::new(
            "Internet",
            "upload",
            Status::Pass,
            format!("{:.1} Mbit/s", speed * 8.0 / 1_000_000.0),
        )
    } else {
        Check::new(
            "Internet",
            "upload",
            Status::Fail,
            "upload test failed or timed out",
        )
    }
}
