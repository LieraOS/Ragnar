// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — printers panel: CUPS helpers
// Copyright (C) 2026 LieraOS Project
//
// lpstat/lpadmin/lpoptions wrappers, the queue list rows
// (default marker, set-default, remove), PPD option parsing and
// the per-queue defaults cards. `lpoptions -o` writes per-user
// defaults CUPS applies to every job of this user.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

use crate::i18n::msgid;
use super::common;

/// lpadmin lives in /usr/sbin, which is not guaranteed on PATH.
const LPADMIN: &str = "/usr/sbin/lpadmin";
/// cups-pdf virtual backend — presence means PDF output is supported.
const PDF_BACKEND: &str = "/usr/lib/cups/backend/cups-pdf";
/// PPD shipped by printer-driver-cups-pdf (exposes tunable options).
const PDF_PPD: &str = "/usr/share/ppd/cups-pdf/CUPS-PDF_opt.ppd";
/// Queue name used when the panel provisions the standard PDF printer.
const PDF_QUEUE: &str = "PDF";
/// cups-pdf daemon config — holds the system-wide output directory.
const PDF_CONF: &str = "/etc/cups/cups-pdf.conf";
/// PPD option keys exposed as editable PDF defaults.
const PDF_OPTIONS: &[&str] = &["PageSize", "Resolution", "PDFVer"];
/// Option keys shown directly on each queue's defaults card —
/// the settings nearly every PPD carries and users actually touch.
const CURATED: &[&str] = &[
    "PageSize",
    "Resolution",
    "PrintQuality",
    "ColorModel",
    "OutputMode",
    "Duplex",
    "InputSlot",
    "MediaType",
];

/// Names of all configured queues.
pub fn queues() -> Vec<String> {
    run("lpstat", &["-p"])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Re-render the printer list rows.
pub fn refresh(card: &gtk4::ListBox) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    let default = default_queue();
    let printers = queues();
    if printers.is_empty() {
        card.append(&common::row(
            &gettext("No printers configured"),
            "",
            gtk4::Label::new(None).upcast(),
        ));
        return;
    }
    for name in printers {
        card.append(&printer_row(&name, name == default, card));
    }
}

/// One printer row: name (+ default marker), set-default and remove.
fn printer_row(name: &str, is_default: bool, card: &gtk4::ListBox) -> gtk4::ListBoxRow {
    let controls = Box::new(Orientation::Horizontal, 6);
    controls.set_valign(gtk4::Align::Center);

    if !is_default {
        let def = gtk4::Button::with_label(&gettext("Set default"));
        let (n, c) = (name.to_string(), card.clone());
        def.connect_clicked(move |_| {
            let _ = Command::new("lpoptions").args(["-d", &n]).status();
            refresh(&c);
        });
        controls.append(&def);
    }

    let del = gtk4::Button::from_icon_name("user-trash-symbolic");
    del.set_has_frame(false);
    del.set_tooltip_text(Some(&gettext("Remove printer")));
    {
        let (n, c) = (name.to_string(), card.clone());
        del.connect_clicked(move |_| {
            let _ = Command::new(LPADMIN).args(["-x", &n]).status();
            refresh(&c);
        });
    }
    controls.append(&del);

    let sub = if is_default { gettext("Default printer") } else { String::new() };
    common::row(name, &sub, controls.upcast())
}

/// Defaults card for a real queue: the curated options inline, the
/// remaining PPD options tucked into an "All options" expander.
pub fn defaults_card(queue: &str) -> gtk4::ListBox {
    let card = common::card();
    let options = ppd_options(queue, &[]);
    if options.is_empty() {
        card.append(&common::row(
            &gettext("No adjustable options reported by the driver"),
            "",
            gtk4::Label::new(None).upcast(),
        ));
        return card;
    }
    let (curated, rest): (Vec<_>, Vec<_>) = options
        .into_iter()
        .partition(|(key, _, _, _)| CURATED.contains(&key.as_str()));
    for (key, label, choices, current) in curated {
        card.append(&option_row(queue, &key, &label, &choices, &current));
    }
    // The remaining driver-specific keys live in a collapsed
    // expander — power users can still reach them, normal users
    // only ever see the curated set.
    if !rest.is_empty() {
        let inner = common::card();
        for (key, label, choices, current) in rest {
            inner.append(&option_row(queue, &key, &label, &choices, &current));
        }
        let expander = gtk4::Expander::new(Some(&gettext("All options")));
        expander.set_expanded(false);
        expander.set_child(Some(&inner));
        let row = gtk4::ListBoxRow::new();
        row.set_activatable(false);
        row.set_child(Some(&expander));
        card.append(&row);
    }
    card
}

/// PDF queue card: its tunable PPD options plus the output folder.
pub fn pdf_card(queue: &str) -> gtk4::ListBox {
    let card = common::card();
    for (key, label, choices, current) in ppd_options(queue, PDF_OPTIONS) {
        card.append(&option_row(queue, &key, &label, &choices, &current));
    }
    card.append(&common::row(
        &gettext("Output folder"),
        &gettext("System-wide — set in /etc/cups/cups-pdf.conf"),
        out_dir_label(),
    ));
    card
}

/// Dropdown row writing a per-user default via `lpoptions -o`.
fn option_row(
    queue: &str,
    key: &str,
    label: &str,
    choices: &[String],
    current: &str,
) -> gtk4::ListBoxRow {
    let dd = gtk4::DropDown::from_strings(&choices.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = choices.iter().position(|c| c == current) {
        dd.set_selected(i as u32);
    }
    dd.set_valign(gtk4::Align::Center);
    {
        let (q, k, owned) = (queue.to_string(), key.to_string(), choices.to_vec());
        dd.connect_selected_notify(move |d| {
            if let Some(v) = owned.get(d.selected() as usize) {
                let _ = Command::new("lpoptions")
                    .args(["-p", &q, "-o", &format!("{k}={v}")])
                    .status();
            }
        });
    }
    // Unknown PPD keys have no translation — gettext("") would
    // return the .po header. Fall back to the driver label as the
    // title and show the technical key underneath.
    let known = opt_label(key);
    let (title, sub) = if known.is_empty() {
        (label.to_string(), key.to_string())
    } else {
        (gettext(known), label.to_string())
    };
    common::row(&title, &sub, dd.upcast())
}

/// Name of the queue that routes to the cups-pdf backend, if any.
pub fn pdf_queue() -> Option<String> {
    run("lpstat", &["-v"]).and_then(|s| {
        s.lines().find_map(|l| {
            let toks: Vec<&str> = l.split_whitespace().collect();
            if toks.last()?.starts_with("cups-pdf") {
                toks.iter()
                    .find(|t| t.ends_with(':'))
                    .map(|t| t.trim_end_matches(':').to_string())
            } else {
                None
            }
        })
    })
}

/// Create the standard PDF queue unless one already exists.
pub fn ensure_pdf_printer() {
    if !std::path::Path::new(PDF_BACKEND).exists() || !std::path::Path::new(PDF_PPD).exists() {
        return;
    }
    if pdf_queue().is_none() {
        let _ = Command::new(LPADMIN)
            .args(["-p", PDF_QUEUE, "-E", "-v", "cups-pdf:/", "-P", PDF_PPD])
            .status();
    }
}

/// Name of the system default queue (empty when none is set).
fn default_queue() -> String {
    run("lpstat", &["-d"])
        .and_then(|s| s.split_once(':').map(|(_, n)| n.trim().to_string()))
        .unwrap_or_default()
}

/// Parse `lpoptions -l` into (key, ppd label, choices, current);
/// `only` restricts to the listed keys (empty = all options).
fn ppd_options(queue: &str, only: &[&str]) -> Vec<(String, String, Vec<String>, String)> {
    let out = run("lpoptions", &["-p", queue, "-l"]).unwrap_or_default();
    out.lines()
        .filter_map(|l| {
            let (key_label, vals) = l.split_once(':')?;
            let (key, label) = key_label.split_once('/')?;
            if !only.is_empty() && !only.contains(&key) {
                return None;
            }
            let mut choices = Vec::new();
            let mut current = String::new();
            for c in vals.split_whitespace() {
                if let Some(v) = c.strip_prefix('*') {
                    current = v.to_string();
                }
                choices.push(c.trim_start_matches('*').to_string());
            }
            Some((key.to_string(), label.trim().to_string(), choices, current))
        })
        .collect()
}

/// Translatable label for a known PPD option key.
fn opt_label(key: &str) -> &'static str {
    match key {
        "PageSize" => msgid("Paper size"),
        "Resolution" => msgid("Resolution"),
        "PrintQuality" => msgid("Print quality"),
        "ColorModel" => msgid("Color model"),
        "OutputMode" => msgid("Output mode"),
        "Duplex" => msgid("Duplex"),
        "InputSlot" => msgid("Paper tray"),
        "MediaType" => msgid("Media type"),
        "PDFVer" => msgid("PDF version"),
        _ => "",
    }
}

/// Label showing where cups-pdf drops its files (resolved ${HOME}).
fn out_dir_label() -> gtk4::Widget {
    let raw = std::fs::read_to_string(PDF_CONF)
        .ok()
        .and_then(|s| {
            s.lines()
                .map(str::trim)
                .find(|l| l.starts_with("Out "))
                .map(|l| l[4..].trim().to_string())
        })
        .unwrap_or_else(|| "${HOME}/PDF".to_string());
    let shown = if let Some(home) = std::env::var_os("HOME") {
        raw.replace("${HOME}", &home.to_string_lossy())
    } else {
        raw
    };
    let l = gtk4::Label::new(Some(&shown));
    l.add_css_class("dim-label");
    l.set_valign(gtk4::Align::Center);
    l.upcast()
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
