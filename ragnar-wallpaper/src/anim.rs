// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-wallpaper — animated-wallpaper flag
//
// ragnar-settings writes the toggle to
// ~/.config/ragnar-settings/wallpaper-animated ("true"/"false").
// Same mechanism as the per-monitor map: this service watches the
// file and re-applies, which swaps swaybg for ragnar-livewall or
// back.

use gio::prelude::*;
use std::path::PathBuf;

/// Path of the flag file written by ragnar-settings.
pub fn path() -> PathBuf {
    let mut p = glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("wallpaper-animated");
    p
}

/// Whether the user wants the animated wallpaper.
pub fn enabled() -> bool {
    std::fs::read_to_string(path())
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// Path of the per-image effects file written by the
/// ragnar-settings effects editor — changes re-apply so a
/// running ragnar-livewall respawns with new parameters.
pub fn effects_path() -> PathBuf {
    let mut p = glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("livewall-effects.ini");
    p
}

/// Re-runs the callback whenever the flag file or the effects
/// file changes.
pub fn watch<F: Fn() + 'static>(cb: F) {
    let cb = std::rc::Rc::new(cb);
    for p in [path(), effects_path()] {
        let file = gio::File::for_path(p);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir.path().unwrap_or_default());
        }
        if let Ok(monitor) =
            file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
        {
            // Keep the monitor alive for the process lifetime.
            std::mem::forget(monitor.clone());
            let cb = cb.clone();
            monitor.connect_changed(move |_, _, _, ev| {
                if ev != gio::FileMonitorEvent::Deleted {
                    cb();
                }
            });
        }
    }
}
