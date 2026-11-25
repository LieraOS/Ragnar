// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — idle dim/undim via sysfs + logind
//
// Implements the Dim()/Undim() endpoints that the swayidle timeout
// hooks call. Mirrors the legacy behaviour: save the level on dim,
// restore on undim, only dim when the current level is brighter
// than the idle target. Writes go through logind's
// Session.SetBrightness — unprivileged, like ragnar-brightness-helper.

use gio::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const SYSFS: &str = "/sys/class/backlight";

/// Level saved on the last Dim so Undim can restore it exactly.
static SAVED: AtomicU64 = AtomicU64::new(0);

/// First backlight device with a nonzero max_brightness; None on
/// desktops/VMs (dim is then a no-op, same as the legacy path).
fn device() -> Option<String> {
    fs::read_dir(SYSFS).ok()?.flatten().find_map(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        (read(&n, "max_brightness") > 0).then_some(n)
    })
}

/// Read an unsigned integer sysfs attribute of a backlight device.
fn read(dev: &str, attr: &str) -> u64 {
    fs::read_to_string(Path::new(SYSFS).join(dev).join(attr))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Write a brightness level via logind on the caller's session.
fn write(dev: &str, value: u64) {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) else {
        return;
    };
    let _ = conn.call_sync(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1/session/self",
        "org.freedesktop.login1.Session",
        "SetBrightness",
        Some(&glib::Variant::tuple_from_iter([
            "backlight".to_variant(),
            dev.to_variant(),
            (value as u32).to_variant(),
        ])),
        None,
        gio::DBusCallFlags::NONE,
        10_000,
        gio::Cancellable::NONE,
    );
}

/// Dim(): save the current level, drop to `idle_percent` when brighter.
pub fn dim(idle_percent: i32) {
    let Some(dev) = device() else { return };
    let max = read(&dev, "max_brightness");
    let cur = read(&dev, "brightness");
    SAVED.store(cur, Ordering::Relaxed);
    let target = idle_percent.max(0) as u64 * max / 100;
    if cur > target {
        write(&dev, target);
    }
}

/// Undim(): restore the level saved by the last Dim().
pub fn undim() {
    let Some(dev) = device() else { return };
    write(&dev, SAVED.load(Ordering::Relaxed));
}
