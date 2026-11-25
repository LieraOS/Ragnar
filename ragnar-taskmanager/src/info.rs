// SPDX-License-Identifier: GPL-3.0-or-later
//! Info tab — the centred overview card from the original: app name,
//! version, description, key features and the copyright/attribution
//! block. Static content; live facts live on the other tabs.

use gettextrs::gettext as tr;

/// Build the info card.
pub fn page() -> gtk4::Widget {
    let scroll = gtk4::ScrolledWindow::new();
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    card.set_halign(gtk4::Align::Center);
    card.set_valign(gtk4::Align::Center);
    card.set_margin_top(48); card.set_margin_bottom(48);
    card.set_width_request(560);

    let title = gtk4::Label::new(Some("Ragnar Task Manager"));
    title.add_css_class("title-1");
    let version = gtk4::Label::new(Some(&tr("Version 0.1.0")));
    version.add_css_class("dim-label");
    let desc = gtk4::Label::new(Some(&tr(
        "A modern, user-friendly system task manager for Linux\n\
         designed to help you monitor and manage your system efficiently.",
    )));
    desc.set_justify(gtk4::Justification::Center);
    card.append(&title); card.append(&version); card.append(&desc);

    let feats = gtk4::Label::new(Some(&format!("{}:\n\n\
        • {} — {}\n\
        • {} — {}\n\
        • {} — {}\n\
        • {} — {}\n\
        • {} — {}\n\
        • {} — {}",
        tr("Key Features"),
        tr("Process Management"), tr("Monitor and control running applications"),
        tr("Performance Monitoring"), tr("Real-time CPU, memory, disk, network, and GPU tracking"),
        tr("System Information"), tr("Detailed hardware and software overview"),
        tr("Startup Control"), tr("Manage applications that launch at boot"),
        tr("Service Management"), tr("Control system services easily"),
        tr("Global Hotkey"), tr("Launch instantly with Ctrl+Shift+Esc"))));
    feats.set_halign(gtk4::Align::Start);
    feats.set_xalign(0.0);
    card.append(&feats);

    let copy = gtk4::Label::new(Some(
        "Copyright © 2025 Jörg Schröder\nWebsite: lieraos.com | License: GPL v3.0"));
    copy.set_halign(gtk4::Align::Start);
    copy.set_xalign(0.0);
    copy.add_css_class("dim-label");
    card.append(&copy);

    // System facts — same data the System tab of the original showed.
    use libadwaita::prelude::*;
    let g = libadwaita::PreferencesGroup::builder()
        .title(&tr("System Information")).build();
    for (k, v) in [
        (tr("Hostname"), read("/proc/sys/kernel/hostname")),
        (tr("Operating System"), os_name()),
        (tr("Kernel"), read("/proc/sys/kernel/osrelease")),
        (tr("Architecture"), std::env::consts::ARCH.to_string()),
        (tr("Cores"), crate::procfs::num_cpus().to_string()),
        (tr("Memory"), crate::util::kb_human(crate::sysstat::meminfo().total)),
    ] {
        g.add(&libadwaita::ActionRow::builder().title(&k).subtitle(&v)
            .subtitle_selectable(true).build());
    }
    let up = libadwaita::ActionRow::builder().title(&tr("Uptime")).build();
    let load = libadwaita::ActionRow::builder().title(&tr("Load Average")).build();
    g.add(&up); g.add(&load);
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let s = crate::sysstat::uptime_secs() as u64;
        let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
        up.set_subtitle(&if d > 0 { format!("{d}d {h}h {m}m") } else { format!("{h}h {m}m") });
        load.set_subtitle(&crate::sysstat::loadavg());
        glib::ControlFlow::Continue
    });
    g.set_margin_top(8);
    g.set_width_request(560);
    card.append(&g);

    outer.append(&card);
    scroll.set_child(Some(&outer));
    scroll.upcast()
}

/// Trimmed contents of a one-line procfs/sysfs file.
fn read(path: &str) -> String {
    std::fs::read_to_string(path).map(|s| s.trim().into()).unwrap_or_default()
}

/// PRETTY_NAME from /etc/os-release, fallback kernel release.
fn os_name() -> String {
    std::fs::read_to_string("/etc/os-release").ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string()))
        .unwrap_or_else(|| read("/proc/sys/kernel/osrelease"))
}
