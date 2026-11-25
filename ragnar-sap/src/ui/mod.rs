// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — GTK4 user interface
//
// Window layer wiring the ported backends (SANE scan, CUPS print,
// image pipeline, PDF tools) into a native GTK4 application.

mod dashboard;
mod pdf_window;
mod prefs_dialog;
mod doc_render;
mod print_only;
mod preview;
mod print_submit;
mod scan_print_ops;
mod scan_print_widgets;
mod scan_print;
mod worker;

use gtk4::prelude::IsA;

pub use dashboard::present_dashboard;

/// Preferences entry point used by the app action.
pub fn prefs_entry(parent: &impl IsA<gtk4::Widget>) {
 prefs_dialog::present_prefs(parent)
}
