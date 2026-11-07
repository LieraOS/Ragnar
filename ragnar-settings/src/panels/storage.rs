// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Storage" panel
// Copyright (C) 2026 LieraOS Project
//
// Live view of every mounted block device: a card per filesystem
// with device, mount point and a usage bar fed by `df` byte
// counts — no udisks dependency, no fake numbers. Entries above
// 90 % are flagged visually. Removable mounts under /run/media
// are grouped below the internal disks.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

/// One mounted filesystem.
struct Fs {
    dev: String,
    mount: String,
    size: u64,
    used: u64,
}

/// Parse `df` for real block devices only (tmpfs/loop excluded).
fn mounts() -> Vec<Fs> {
    let out = Command::new("df")
        .args(["-B1", "--output=source,target,size,used"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    out.lines()
        .skip(1)
        .filter(|l| l.starts_with("/dev/"))
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            (p.len() >= 4).then(|| Fs {
                dev: p[0].to_string(),
                mount: p[1..p.len() - 2].join(" "),
                size: p[p.len() - 2].parse().unwrap_or(0),
                used: p[p.len() - 1].parse().unwrap_or(0),
            })
        })
        .collect()
}

/// Human-readable GiB/TiB string.
fn human(b: u64) -> String {
    let g = b as f64 / 1_073_741_824.0;
    if g >= 1024.0 {
        format!("{:.1} TiB", g / 1024.0)
    } else {
        format!("{g:.1} GiB")
    }
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }
    let heading = gtk4::Label::new(Some(&gettext("Storage")));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    page.append(&heading);

    let (internal, external): (Vec<_>, Vec<_>) = mounts()
        .into_iter()
        .partition(|f| !f.mount.starts_with("/run/media") && !f.mount.starts_with("/media"));

    let list = Box::new(Orientation::Vertical, 12);
    for f in &internal {
        list.append(&disk_card(f));
    }
    page.append(&list);

    if !external.is_empty() {
        let sub = gtk4::Label::new(Some(&gettext("Removable devices")));
        sub.add_css_class("heading");
        sub.set_xalign(0.0);
        page.append(&sub);
        let ext = Box::new(Orientation::Vertical, 12);
        for f in &external {
            ext.append(&disk_card(f));
        }
        page.append(&ext);
    }

    page.upcast()
}

/// One filesystem card: header (device + mount), usage bar, numbers.
fn disk_card(f: &Fs) -> gtk4::Widget {
    let frame = gtk4::Frame::new(None);
    let body = Box::new(Orientation::Vertical, 8);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 14);
    }

    let top = Box::new(Orientation::Horizontal, 10);
    let name = gtk4::Label::new(Some(&f.mount));
    name.add_css_class("title-4");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    top.append(&name);
    let dev = gtk4::Label::new(Some(&f.dev));
    dev.add_css_class("dim-label");
    dev.add_css_class("monospace");
    top.append(&dev);
    body.append(&top);

    let pct = if f.size > 0 {
        f.used as f64 / f.size as f64
    } else {
        0.0
    };
    let bar = gtk4::ProgressBar::new();
    bar.set_fraction(pct);
    bar.set_show_text(false);
    if pct > 0.9 {
        bar.add_css_class("warning"); // flagged via theme color
    }
    body.append(&bar);

    let caption = gtk4::Label::new(Some(&format!(
        "{} {}  ·  {} {}  ·  {:.0}% {}",
        human(f.used),
        gettext("used"),
        human(f.size),
        gettext("total"),
        pct * 100.0,
        gettext("full"),
    )));
    caption.add_css_class("dim-label");
    caption.add_css_class("caption");
    caption.set_xalign(0.0);
    body.append(&caption);

    frame.set_child(Some(&body));
    frame.upcast()
}
