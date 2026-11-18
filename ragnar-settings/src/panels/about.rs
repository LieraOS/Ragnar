//! "About this system" panel — real system information.
//!
//! Works on any host (no Ragnar dependency): operating system from
//! `/etc/os-release`, kernel from `uname`, memory from `/proc/meminfo`.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};

/// Build the about panel widget.
pub fn build() -> gtk4::Widget {
    let container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(32)
        .margin_end(32)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();

    for (label, value) in system_facts() {
        let row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(24)
            .build();
        let key = Label::builder()
            .label(label)
            .xalign(0.0)
            .width_chars(16)
            .build();
        let val = Label::builder()
            .label(&value)
            .xalign(0.0)
            .selectable(true)
            .wrap(true)
            .build();
        row.append(&key);
        row.append(&val);
        container.append(&row);
    }

    container.upcast()
}

/// Collected facts, already localized for display.
fn system_facts() -> Vec<(String, String)> {
    vec![
        (gettextrs::gettext("Operating system"), os_pretty_name()),
        (gettextrs::gettext("Ragnar version"), ragnar_version()),
        (gettextrs::gettext("Kernel"), kernel_release()),
        (gettextrs::gettext("CPU"), cpu_model()),
        (gettextrs::gettext("GPU"), gpu_model()),
        (gettextrs::gettext("Memory"), memory_total()),
        (gettextrs::gettext("Desktop"), desktop_environment()),
        (gettextrs::gettext("Session"), session_type()),
        (gettextrs::gettext("Hostname"), hostname()),
        (gettextrs::gettext("Uptime"), uptime()),
    ]
}

/// Ragnar version from the file ragnar-core ships at
/// /usr/share/ragnar/ragnar-version.xml.
fn ragnar_version() -> String {
    std::fs::read_to_string("/usr/share/ragnar/ragnar-version.xml")
        .ok()
        .and_then(|s| {
            let a = s.find("<str>")? + 5;
            let b = s.find("</str>")?;
            Some(s[a..b].trim().to_string())
        })
        .unwrap_or_else(|| gettextrs::gettext("Unknown"))
}

/// First "model name" line from /proc/cpuinfo.
fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .and_then(|r| r.split(':').nth(1))
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| gettextrs::gettext("Unknown"))
}

/// VGA/3D controller names from lspci, joined with " / ".
fn gpu_model() -> String {
    std::process::Command::new("lspci")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|out| {
            out.lines()
                .filter(|l| l.contains("VGA") || l.contains("3D controller"))
                .filter_map(|l| l.split(": ").nth(1).map(|s| s.trim().to_string()))
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| gettextrs::gettext("Unknown"))
}

/// Wayland/X11 from the environment.
fn session_type() -> String {
    std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| gettextrs::gettext("Unknown"))
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| gettextrs::gettext("Unknown"))
}

/// /proc/uptime → "Xd Yh" / "Xh Ym" / "Xm".
fn uptime() -> String {
    let secs: u64 = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    match (d, h) {
        (d, _) if d > 0 => format!("{d}d {h}h"),
        (_, h) if h > 0 => format!("{h}h {m}m"),
        _ => format!("{m}m"),
    }
}

/// `PRETTY_NAME` from /etc/os-release.
fn os_pretty_name() -> String {
    let contents = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    contents
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| gettextrs::gettext("Unknown"))
}

/// `uname -r` equivalent via /proc.
fn kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| gettextrs::gettext("Unknown"))
}

/// MemTotal converted to GiB with one decimal.
fn memory_total() -> String {
    let contents = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kib: Option<u64> = contents
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next()?.parse().ok());
    match kib {
        Some(k) => format!("{:.1} GiB", k as f64 / 1024.0 / 1024.0),
        None => gettextrs::gettext("Unknown"),
    }
}

/// Current desktop session, or "—" outside one.
fn desktop_environment() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "—".into())
}
