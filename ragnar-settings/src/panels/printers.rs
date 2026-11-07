// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Printers" panel
// Copyright (C) 2026 LieraOS Project
//
// Real CUPS integration via the standard command line tools
// (lpstat / lpinfo / lpadmin / lpoptions) — the user is in the
// lpadmin group, so no privilege elevation is needed. Lists
// configured printers with state, lets one become default,
// removes printers, and adds discovered network/USB printers
// driverless (IPP Everywhere, `-m everywhere`).

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

/// Build the printers panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 16);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    let list = Box::new(Orientation::Vertical, 6);
    page.append(&list);
    refresh(&list);

    page.append(&gtk4::Separator::new(Orientation::Horizontal));

    // -- Add printer ------------------------------------------------------
    let ah = gtk4::Label::new(Some(&gettext("Add printer")));
    ah.add_css_class("heading");
    ah.set_xalign(0.0);
    page.append(&ah);

    let discovered = discover();
    let arow = Box::new(Orientation::Horizontal, 10);
    let name_entry = gtk4::Entry::builder()
        .placeholder_text(gettext("Printer name"))
        .hexpand(true)
        .build();
    arow.append(&name_entry);

    let mut uris: Vec<String> = discovered;
    if uris.is_empty() {
        uris.push(gettext("No printers discovered").to_string());
    }
    let dd = gtk4::DropDown::from_strings(&uris.iter().map(String::as_str).collect::<Vec<_>>());
    dd.set_hexpand(true);
    arow.append(&dd);

    let add = gtk4::Button::with_label(&gettext("Add"));
    {
        let (l, ur, ne) = (list.clone(), uris.clone(), name_entry.clone());
        add.connect_clicked(move |_| {
            let uri = match ur.get(dd.selected() as usize) {
                Some(u) if u.contains("://") || u.starts_with("usb") || u.starts_with("ipp") => u,
                _ => return,
            };
            let name = ne.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            add_printer(&name, uri);
            refresh(&l);
        });
    }
    arow.append(&add);
    page.append(&arow);

    let hint = gtk4::Label::new(Some(&gettext(
        "Printers are added driverless (IPP Everywhere). Network \
         printers need to be reachable on the network.",
    )));
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    page.append(&hint);

    page.upcast()
}

/// Re-render the printer list rows.
fn refresh(list: &Box) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    let default = run("lpstat", &["-d"])
        .and_then(|s| s.split(':').nth(1).map(str::trim).map(str::to_string))
        .unwrap_or_default();
    let printers = run("lpstat", &["-p"])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if printers.is_empty() {
        let none = gtk4::Label::new(Some(&gettext("No printers configured")));
        none.add_css_class("dim-label");
        none.set_xalign(0.0);
        list.append(&none);
        return;
    }
    for name in printers {
        let row = Box::new(Orientation::Horizontal, 10);
        let is_default = name == default;
        let label = if is_default {
            format!("{name}  ✓")
        } else {
            name.clone()
        };
        let l = gtk4::Label::new(Some(&label));
        l.set_xalign(0.0);
        l.set_hexpand(true);
        row.append(&l);

        if !is_default {
            let def = gtk4::Button::with_label(&gettext("Set default"));
            let (n, ls) = (name.clone(), list.clone());
            def.connect_clicked(move |_| {
                let _ = Command::new("lpoptions").args(["-d", &n]).status();
                refresh(&ls);
            });
            row.append(&def);
        }

        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        del.set_has_frame(false);
        del.set_tooltip_text(Some(&gettext("Remove printer")));
        let (n, ls) = (name.clone(), list.clone());
        del.connect_clicked(move |_| {
            let _ = Command::new("lpadmin").args(["-x", &n]).status();
            refresh(&ls);
        });
        row.append(&del);
        list.append(&row);
    }
}

/// Discovered printer URIs (`lpinfo -v` lines: "<scheme> <uri>").
fn discover() -> Vec<String> {
    run("lpinfo", &["-v"])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
                .filter(|u| !u.starts_with("beh") && !u.starts_with("socket://"))
                .collect()
        })
        .unwrap_or_default()
}

/// Add a printer driverless (IPP Everywhere).
fn add_printer(name: &str, uri: &str) {
    let _ = Command::new("lpadmin")
        .args(["-p", name, "-E", "-v", uri, "-m", "everywhere"])
        .status();
}

/// Run a tool, return trimmed stdout on success.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).output().ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}
