// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — user groups dialog
// Copyright (C) 2026 LieraOS Project
//
// "Edit groups" modal: checkable list of relevant system groups.
// Apply computes the supplementary-group diff and writes it through
// `pkexec usermod -G` — one admin authentication via the running
// ragnar-polkit-dialog agent. Like everywhere on Linux, membership
// changes take effect on the user's next login.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

/// Groups worth offering: desktop-relevant system groups plus
/// whatever the user is already in (never hide their memberships).
fn candidate_groups(current: &[String]) -> Vec<String> {
    const KNOWN: &[&str] = &[
        "sudo",
        "adm",
        "lpadmin",
        "netdev",
        "plugdev",
        "users",
        "audio",
        "video",
        "bluetooth",
        "cdrom",
        "dialout",
        "docker",
        "kvm",
        "libvirt",
        "vboxusers",
        "wireshark",
        "fuse",
        "input",
        "render",
        "sambashare",
        "gamemode",
        "autologin",
        "wireshark",
    ];
    let existing: Vec<String> = Command::new("getent")
        .arg("group")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split(':').next().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut out: Vec<String> = KNOWN
        .iter()
        .filter(|g| existing.iter().any(|e| e == *g))
        .map(|g| g.to_string())
        .chain(current.iter().cloned())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Current supplementary groups of `user` (primary group excluded).
fn current_groups(user: &str) -> Vec<String> {
    let primary = Command::new("id")
        .args(["-gn", user])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    Command::new("id")
        .args(["-nG", user])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .filter(|g| *g != primary)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Render group names as chips inside a FlowBox.
pub fn chips(list: &gtk4::FlowBox, groups: &[String]) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    for g in groups {
        let l = gtk4::Label::new(Some(g));
        l.add_css_class("chip");
        l.set_margin_start(6);
        l.set_margin_end(6);
        l.set_margin_top(2);
        l.set_margin_bottom(2);
        let frame = gtk4::Frame::new(None);
        frame.set_child(Some(&l));
        list.append(&frame);
    }
}

/// Group memberships of `user` — read via `id`, used by the panel.
pub fn memberships(user: &str) -> Vec<String> {
    current_groups(user)
}

/// Open the group-edit dialog for `user`; `chips_box` is re-rendered
/// afterwards so the panel reflects the change immediately.
pub fn dialog(anchor: &gtk4::Button, user: &str, chips_box: &gtk4::FlowBox) {
    let win = gtk4::Window::builder()
        .title(gettext("Edit groups"))
        .modal(true)
        .default_width(320)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 10);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }

    let current = current_groups(user);
    let mut checks: Vec<(String, gtk4::CheckButton)> = Vec::new();
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_min_content_height(260);
    scroll.set_max_content_height(360);
    let list = Box::new(Orientation::Vertical, 6);
    for g in candidate_groups(&current) {
        let cb = gtk4::CheckButton::with_label(&g);
        cb.set_active(current.contains(&g));
        checks.push((g, cb.clone()));
        list.append(&cb);
    }
    scroll.set_child(Some(&list));
    body.append(&scroll);

    let note = gtk4::Label::new(Some(&gettext(
        "Group changes take effect at the next login.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    body.append(&note);

    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let apply = gtk4::Button::with_label(&gettext("Apply"));
    apply.add_css_class("suggested-action");
    btns.append(&cancel);
    btns.append(&apply);
    body.append(&btns);

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    let (w, u, cb) = (win.clone(), user.to_string(), chips_box.clone());
    apply.connect_clicked(move |_| {
        let wanted: Vec<String> = checks
            .iter()
            .filter(|(_, c)| c.is_active())
            .map(|(g, _)| g.clone())
            .collect();
        // usermod -G replaces the supplementary set wholesale.
        let _ = Command::new("pkexec")
            .args(["/usr/sbin/usermod", "-G", &wanted.join(","), &u])
            .status();
        chips(&cb, &current_groups(&u));
        w.close();
    });

    win.set_child(Some(&body));
    win.present();
}
