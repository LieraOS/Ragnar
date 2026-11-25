// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Printers" panel
// Copyright (C) 2026 LieraOS Project
//
// Manage view over the real CUPS state via the standard command line
// tools (lpstat / lpadmin / lpoptions) — the user is in the lpadmin
// group, so no privilege elevation is needed. Every configured queue
// can become default or be removed and its PPD defaults are editable
// (curated options inline, everything else under "All options").
// Adding printers is deliberately NOT offered here — driver/queue
// setup happens elsewhere. The standard PDF printer is ensured on
// open and its per-user default options are editable as well.
// The scanner section stores per-device scan defaults in
// ~/.config/ragnar-sap/preferences.ini — the file ragnar-sap reads
// for its combo pre-selection.

mod cups;
mod scanner;

use gettextrs::gettext;
use gtk4::prelude::*;

use super::common;

/// Build the printers panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Printers"));

    // Provision the standard PDF printer before listing so it shows
    // up like any other queue — but only when the backend exists.
    cups::ensure_pdf_printer();

    page.append(&common::section(&gettext("Printers")));
    let card = common::card();
    cups::refresh(&card);
    page.append(&card);
    page.append(&common::note(&gettext(
        "Printers are set up through their drivers — this list shows \
         what is already configured.",
    )));

    // Per-queue default options for every real (non-PDF) queue.
    let pdf = cups::pdf_queue();
    for queue in cups::queues() {
        if pdf.as_deref() == Some(queue.as_str()) {
            continue;
        }
        page.append(&common::section(&format!("{queue} — {}", gettext("Defaults"))));
        page.append(&cups::defaults_card(&queue));
    }

    if let Some(queue) = pdf {
        page.append(&common::section(&gettext("PDF printer")));
        page.append(&cups::pdf_card(&queue));
    }

    page.append(&common::section(&gettext("Scanners")));
    page.append(&scanner::card());
    page.append(&common::note(&gettext(
        "Scan defaults are stored per device and used by Ragnar SAP.",
    )));

    page.upcast()
}
