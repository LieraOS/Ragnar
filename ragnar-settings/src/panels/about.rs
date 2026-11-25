// Copyright (C) Jörg Schröder
//! "About this system" panel — real system information plus the
//! LieraOS story (which quietly hints at the `ragnarok` easter egg).
//!
//! Works on any host (no Ragnar dependency): operating system from
//! `/etc/os-release`, kernel from `uname`, memory from `/proc/meminfo`.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Separator};

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
        .valign(gtk4::Align::Start)
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

    let sep = Separator::builder()
        .orientation(Orientation::Horizontal)
        .margin_top(24)
        .margin_bottom(8)
        .build();
    container.append(&sep);
    container.append(&story_section());

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

/// The LieraOS origin story, rendered as wrapped paragraphs below the
/// facts. Every string goes through gettext.
fn story_section() -> gtk4::Widget {
    use gettextrs::gettext;

    let section = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();

    let heading = Label::builder()
        .label(gettext("The story of LieraOS"))
        .xalign(0.0)
        .build();
    heading.add_css_class("title-3");
    section.append(&heading);

    for text in story_paragraphs() {
        let p = Label::builder()
            .label(&text)
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .max_width_chars(80)
            .build();
        if text.starts_with("<i>") {
            p.set_use_markup(true);
        }
        section.append(&p);
    }

    section.upcast()
}

/// Story paragraphs, localized. The last one is the lore line that
/// hints at the `ragnarok` command without naming it.
fn story_paragraphs() -> Vec<String> {
    use gettextrs::gettext;
    vec![
        gettext("More than thirty years ago it began with the classic distributions: SuSE, Red Hat, Mandrake — tried them all, none stayed. Ubuntu was fine for a while, until GNOME 3. After years of wandering I landed on Debian — and stayed. Yet one question never let go: why did Debian, the foundation of so many distributions, always stand on the sidelines itself?"),
        gettext("In my view it came down to the update philosophy. So I first built an entire system from scratch — my own package manager, no base at all. It worked, but it was too much for one person alone. Then I returned to Debian — not because Debian was bad, quite the opposite: I still like Debian today, and countless distributions rightly build upon it. I began shaping Debian into what I wanted: a good mix of stability and modern software, fit for gaming too, without giving up current drivers. Small changes here, my own programs there — tools that work the way they should, instead of needing two programs for one task."),
        gettext("That became a distribution: Liera — from Swedish, \"ally\", at its core a Debian derivative. And because my own programs had long grown beyond a mere distribution, LieraOS came to be — the entire open source world around Liera: the distribution, the desktop, the tools. The OS does not stand for \"Operating System\" but for Open Source. Everything is licensed under the GPL."),
        gettext("The tools' names followed the Norse world that Liera belongs to: Ragnar, the desktop. Odin, the compositor — still in the making, yet written in Rust like nearly all of LieraOS; the name was pure consequence, for just as Odin laid down the order of things in the myths, the compositor lays down how everything appears. Valkyrie, the file manager — a name that simply belonged to this world. Heimdall, guardian of the Bifröst — here guardian of the network. Skald, the poet and singer of the Norse world — here for sound and music. Raven, the messenger who brings word — as ravens always have."),
        format!("<i>{}</i>", gettext("And when the twilight of the gods comes, every error shall be counted and shown.")),
    ]
}
