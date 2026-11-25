// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Process classification — ports the original Qt app's App/
//! Background/System split: a process is an *application* when it
//! belongs to a real user, uses meaningful memory and shows GUI
//! characteristics in its command line; a *background* process is
//! user-owned without GUI traits; everything else is *system*.
//! App grouping normalises well-known names (firefox → "Firefox").

use crate::procfs::Process;

/// Display category of a process, matching the classic three-way grouping.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category { App, Background, System }

/// Minimum RSS for an app candidate — filters daemon helper noise.
const APP_MIN_RSS_KB: u64 = 8 * 1024;

/// Well-known name → display name; first match wins.
const APP_NAMES: &[(&str, &str)] = &[
    ("firefox", "Firefox"), ("chrome", "Chrome"), ("chromium", "Chromium"),
    ("opera", "Opera"), ("msedge", "Edge"), ("brave", "Brave"),
    ("vscode", "VS Code"), ("code", "VS Code"), ("windsurf", "Windsurf"),
    ("ragnar-taskbar", "Ragnar Taskbar"), ("ragnar-software", "Ragnar Software"),
    ("ragnar-view", "Ragnar View"), ("valkyrie", "Valkyrie"),
    ("nautilus", "Files"), ("dolphin", "Dolphin"),
    ("gimp", "GIMP"), ("inkscape", "Inkscape"), ("vlc", "VLC"),
    ("mpv", "mpv"), ("obs", "OBS Studio"), ("thunderbird", "Thunderbird"),
];

/// Kernel-thread / daemon name prefixes that are never user apps.
const SYS_NAMES: &[&str] = &[
    "kworker", "ksoftirqd", "migration", "kswapd", "rcu_", "kcompactd",
    "khungtaskd", "kdevtmpfs", "kthreadd", "systemd", "cron", "dbus",
    "networkmanager", "pulseaudio", "pipewire", "xorg",
];

/// Users that never own GUI apps.
const SYS_USERS: &[&str] = &[
    "root", "daemon", "bin", "sys", "sync", "games", "man", "lp", "mail",
    "news", "uucp", "proxy", "www-data", "backup", "list", "irc",
    "gnats", "nobody",
];

/// True for kernel/daemon names by prefix or substring patterns.
fn is_system_name(name: &str) -> bool {
    let n = name.to_lowercase();
    SYS_NAMES.iter().any(|p| n.starts_with(p))
        || n.contains("daemon") || n.contains("kernel")
        || n.contains("kthread") || n.contains("irq")
}

/// True for uid-0 and classic system accounts.
fn is_system_user(user: &str) -> bool {
    let u = user.to_lowercase();
    SYS_USERS.iter().any(|p| u == *p) || u.starts_with("systemd-")
        || u.parse::<u32>().is_ok_and(|u| u < 1000)
}

/// GUI hints in the command line — display, toolkit, browser patterns.
fn has_gui_traits(p: &Process) -> bool {
    let c = p.cmdline.to_lowercase();
    let n = p.name.to_lowercase();
    APP_NAMES.iter().any(|(k, _)| n.contains(k))
        || c.contains("wayland") || c.contains("--display")
        || c.contains("libgtk") || c.contains("libqt") || c.contains("libx11")
        || c.contains("libwayland") || c.contains("electron")
        || c.contains("--gui") || c.contains("browser")
}

/// Classify one process — mirrors the original's category logic.
pub fn classify(p: &Process) -> Category {
    if is_system_name(&p.name) || is_system_user(&p.user) {
        Category::System
    } else if p.rss_kb >= APP_MIN_RSS_KB && has_gui_traits(p) {
        Category::App
    } else {
        Category::Background
    }
}

/// Normalised display name for app grouping ("firefox-esr" → "Firefox").
pub fn app_name(p: &Process) -> String {
    let n = p.name.to_lowercase();
    for (key, disp) in APP_NAMES {
        if n.contains(key) { return disp.to_string() }
    }
    // Capitalise the cleaned binary name, cut common suffixes.
    let base = p.name.trim_end_matches(".bin").trim_end_matches("-bin");
    let mut c = base.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_else(|| base.to_string())
}
