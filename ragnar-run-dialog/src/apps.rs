// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-run-dialog — application enumeration and search
//
// Reads the desktop-file database through GIO once per invocation.
// The dialog is short-lived (open → pick → exit), so caching across
// launches is not worth a daemon; enumeration of a few hundred
// desktop entries takes single-digit milliseconds.

use crate::model::{score_fields, AppEntry};
use gtk4::gio;
use gtk4::prelude::*;

/// Collect every desktop entry that belongs in a launcher, sorted
/// by (localized) name for the unfiltered initial view.
pub fn installed() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = gio::AppInfo::all()
        .into_iter()
        // Hidden/NoDisplay entries still launch, they just do not list.
        .filter(|a| a.should_show())
        .map(|info| AppEntry {
            name: info.display_name().to_string(),
            description: description_of(&info),
            extra: extra_haystack(&info),
            info,
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Second line under the app name: Comment=, falling back to
/// GenericName= (DesktopAppInfo-only field).
fn description_of(info: &gio::AppInfo) -> String {
    if let Some(d) = info.description() {
        return d.to_string();
    }
    info.downcast_ref::<gio::DesktopAppInfo>()
        .and_then(|d| d.generic_name())
        .map(|g| g.to_string())
        .unwrap_or_default()
}

/// Lower-priority search material: Keywords= and the Exec= line, so
/// "gimp" finds the app whose name is localized differently, and
/// power users can match on the real command.
fn extra_haystack(info: &gio::AppInfo) -> String {
    let exe = info.executable().to_string_lossy().into_owned();
    let kw = info
        .downcast_ref::<gio::DesktopAppInfo>()
        .map(|d| d.keywords().join(" "))
        .unwrap_or_default();
    format!("{exe} {kw}")
}

/// Indices into `apps` for everything matching `query`, best score
/// first. The caller caps the result count before building widgets.
pub fn search(apps: &[AppEntry], query: &str) -> Vec<usize> {
    let mut scored: Vec<(usize, i32)> = apps
        .iter()
        .enumerate()
        .filter_map(|(i, e)| score_fields(&e.name, &e.description, &e.extra, query).map(|s| (i, s)))
        .collect();
    scored.sort_by_key(|&(_, s)| s);
    scored.into_iter().map(|(i, _)| i).collect()
}
