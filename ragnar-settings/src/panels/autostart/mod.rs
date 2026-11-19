// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Autostart & environment" panel (advanced)
// Copyright (C) 2026 LieraOS Project
//
// Two plain-text session files, edited as lists: the autostart
// file (programs run at login) and the environment file (session
// KEY=VALUE pairs). Neither is XML — both keep comments and
// blank lines verbatim; edits rewrite only the touched line.
// No compositor reload: autostart runs at next login, environment
// changes reach newly started applications.

mod env;
mod programs;

use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use std::path::PathBuf;

/// Path of a file inside the labwc config dir (next to rc.xml).
fn file_path(name: &str) -> PathBuf {
    gtk4::glib::user_config_dir()
        .join("ragnar-desktop")
        .join("labwc")
        .join(name)
}

/// Read a session file as lines — empty when absent.
fn read_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(file_path(name))
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Write all lines back, creating the file when missing.
fn write_lines(name: &str, lines: &[String]) {
    let _ = std::fs::write(file_path(name), lines.join("\n") + "\n");
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Autostart & environment"));

    let s = common::section(&gettext("Autostart"));
    s.append(&programs::card());
    s.append(&common::note(&gettext(
        "Commands run once at the next login — editing here does \
         not start them now.",
    )));
    page.append(&s);

    let s = common::section(&gettext("Environment"));
    s.append(&env::card());
    s.append(&common::note(&gettext(
        "Variables apply to applications started after the change. \
         Locked entries are maintained by the desktop session and \
         would be overwritten.",
    )));
    page.append(&s);

    page.upcast()
}
