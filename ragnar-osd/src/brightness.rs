// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-osd — the brightness watcher
//
// Watches /sys/class/backlight/*/brightness and shows the OSD when
// the level changes — the same job the legacy OSDKeys did via
// BrightnessManager, but straight against sysfs so it works wherever
// the brightness keys (media-keys plugin → logind/kernel) actually
// write. Absent backlight devices (desktops) simply produce no watch.

use crate::window;
use gio::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;

thread_local! {
    /// Last fraction per device — dedupe so unrelated writes don't
    /// re-trigger the OSD.
    static LAST: RefCell<HashMap<String, i64>> = RefCell::new(HashMap::new());
}

/// Sets up file monitors on every present backlight device.
pub fn watch() {
    let Ok(entries) = fs::read_dir("/sys/class/backlight") else {
        return;
    };
    for entry in entries.flatten() {
        let bright = entry.path().join("brightness");
        let max = entry.path().join("max_brightness");
        if !bright.exists() || !max.exists() {
            continue;
        }
        let dev = entry.file_name().to_string_lossy().to_string();
        seed(&dev, &bright, &max);
        monitor(&dev, bright, max);
    }
}

/// Records the initial level without showing the OSD — matches the
/// legacy guard against the startup event.
fn seed(dev: &str, bright: &std::path::Path, max: &std::path::Path) {
    if let Some(frac) = fraction(bright, max) {
        LAST.with(|l| {
            l.borrow_mut()
                .insert(dev.to_string(), (frac * 1000.0) as i64)
        });
    }
}

/// Attaches a FileMonitor to the brightness file; changes re-evaluate
/// the fraction and show the OSD.
fn monitor(dev: &str, bright: std::path::PathBuf, max: std::path::PathBuf) {
    let dev = dev.to_string();
    let file = gio::File::for_path(&bright);
    let Ok(mon) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) else {
        return;
    };
    mon.connect_changed(move |_, _, _, event| {
        if event != gio::FileMonitorEvent::ChangesDoneHint {
            return;
        }
        let Some(frac) = fraction(&bright, &max) else {
            return;
        };
        let milli = (frac * 1000.0) as i64;
        let changed = LAST.with(|l| {
            let mut l = l.borrow_mut();
            if l.get(&dev) == Some(&milli) {
                false
            } else {
                l.insert(dev.clone(), milli);
                true
            }
        });
        if changed {
            window::show(Some(icon_for(frac)), None, Some(frac));
        }
    });
    std::mem::forget(mon);
}

/// Reads brightness/max_brightness and returns the fraction.
fn fraction(bright: &std::path::Path, max: &std::path::Path) -> Option<f64> {
    let b: f64 = fs::read_to_string(bright).ok()?.trim().parse().ok()?;
    let m: f64 = fs::read_to_string(max).ok()?.trim().parse().ok()?;
    (m > 0.0).then(|| (b / m).clamp(0.0, 1.0))
}

/// Icon name for the level — same tiers as the legacy service.
fn icon_for(frac: f64) -> &'static str {
    if frac <= 0.33 {
        "display-brightness-low-symbolic"
    } else if frac >= 0.67 {
        "display-brightness-high-symbolic"
    } else {
        "display-brightness-symbolic"
    }
}
