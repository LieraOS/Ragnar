// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-run-dialog — starting the picked row

use crate::model::Row;
use gtk4::prelude::*;
use gtk4::{gdk, gio};
use std::process::Command;

/// Launch the row at `index` in the current result set; returns
/// false on failure so the caller keeps the dialog open — a typo
/// should not silently swallow the launcher.
pub fn run(row: &Row, apps: &[crate::model::AppEntry], display: &gdk::Display) -> bool {
    match row {
        Row::App(i) => apps
            .get(*i)
            .map(|e| {
                e.info
                    .launch(&[], Some(&display.app_launch_context()))
                    .is_ok()
            })
            .unwrap_or(false),
        Row::Command(text) => run_command(text),
    }
}

/// Run free-form text. GAppInfo::create_from_commandline handles
/// quoting/args properly; for actual shell syntax (pipes, `~`,
/// globs) it fails to resolve and we fall back to `sh -c`.
fn run_command(text: &str) -> bool {
    let cmd = text.trim();
    if cmd.is_empty() {
        return false;
    }
    let ctx = gio::AppLaunchContext::new();
    if let Ok(info) =
        gio::AppInfo::create_from_commandline(cmd, None, gio::AppInfoCreateFlags::NONE)
    {
        if info.launch(&[], Some(&ctx)).is_ok() {
            return true;
        }
    }
    Command::new("sh").arg("-c").arg(cmd).spawn().is_ok()
}
