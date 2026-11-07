// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-brightness-helper — backlight device discovery via sysfs

use std::fs;
use std::path::Path;

const SYSFS: &str = "/sys/class/backlight";

/// Pick the backlight device to control: first entry in
/// /sys/class/backlight that reports a nonzero max_brightness.
/// Real laptops usually expose exactly one; on desktops/VMs there
/// are none, in which case None is returned (the caller reports
/// the same "no device" case the legacy helper did).
pub fn find() -> Option<String> {
    fs::read_dir(SYSFS)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (max(&name) > 0).then_some(name)
        })
}

/// Read an unsigned integer sysfs attribute of the device.
fn attr(dev: &str, name: &str) -> u64 {
    fs::read_to_string(Path::new(SYSFS).join(dev).join(name))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// max_brightness of the device (0 when unreadable).
pub fn max(dev: &str) -> u64 {
    attr(dev, "max_brightness")
}

/// Current brightness of the device (0 when unreadable).
pub fn current(dev: &str) -> u64 {
    attr(dev, "brightness")
}
