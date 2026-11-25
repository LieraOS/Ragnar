// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — USB sysfs fallback probe,.
//
// Walks /sys/bus/usb/devices reading the udev-exported attributes
// (manufacturer/product/serial/idVendor/idProduct/bDeviceClass) and
// applies the same class + keyword filtering : scanners
// are accepted on Imaging class 0x06, vendor-specific 0xFF, or
// multifunction keywords; Video class 0x0E and cameras are rejected.

use std::collections::HashSet;
use std::path::Path;

/// What kind of device the caller is probing for.
/// `Other` is used by the printer fallback path (hint matching)
/// that lands with DeviceRegistry in the next phase.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeType {
 /// USB scanners / multifunction devices.
 Scanner,
 /// Anything else — matched by hint substring only.
 Other,
}

/// One USB device found in sysfs.
/// vendor/model/serial feed DeviceRegistry metadata in the
/// next phase — kept as part of the probed record.
#[derive(Debug, Clone)]
pub struct UsbDevice {
 /// "usb:<sysfs-dirname>" — matches id scheme.
 pub id: String,
 /// "<manufacturer> <product>" or "USB Device".
 pub display_name: String,
 /// USB vendor string.
 #[allow(dead_code)]
 pub vendor: String,
 /// USB product string.
 #[allow(dead_code)]
 pub model: String,
 /// Serial number, may be empty.
 #[allow(dead_code)]
 pub serial: String,
 /// "vid:pid" hex pair.
 pub usb_id: String,
 /// bDeviceClass hex string.
 pub usb_class: String,
}

/// Read a sysfs attribute file, trimmed of all whitespace — same
/// behaviour as readFileTrimmed.
fn read_trimmed(dir: &Path, file: &str) -> String {
 std::fs::read_to_string(dir.join(file))
.map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
.unwrap_or_default()
}

/// Case-insensitive substring match; empty needle matches all.
fn matches_hint(haystack: &str, needle: &str) -> bool {
 needle.is_empty() || haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// USB class filter from isValidScannerClass:
/// 0x06 Imaging + 0xFF vendor-specific pass, 0x0E Video is rejected.
fn is_valid_scanner_class(class: &str) -> bool {
 if class.is_empty() {
 return false;
 }
 let c = class.to_lowercase();
 if c == "0e" || c == "0x0e" {
 return false;
 }
 c == "06" || c == "0x06" || c == "ff" || c == "0xff"
}

/// Scanner/MFP classifier —.
fn is_scanner_device(manufacturer: &str, product: &str, class: &str) -> bool {
 if !class.is_empty() && !is_valid_scanner_class(class) {
 return false;
 }
 let combined = format!("{} {}", manufacturer, product).to_lowercase();
 for bad in ["webcam", "camera", "usb camera"] {
 if combined.contains(bad) {
 return false;
 }
 }
 for good in ["scanner", "mfp", "multifunction", "all-in-one", "copier"] {
 if combined.contains(good) {
 return true;
 }
 }
 // Class-0x07 printers only count when multifunction keywords hit.
 if combined.contains("printer") {
 return ["mfp", "multifunction", "all-in-one", "mfc"]
.iter()
.any(|k| combined.contains(k));
 }
 false
}

/// Walk sysfs and return matching USB devices, deduplicated by
/// vid:pid — identical to dedupe logic.
pub fn probe(probe_type: ProbeType, hint: &str) -> Vec<UsbDevice> {
 let usb_root = Path::new("/sys/bus/usb/devices");
 let mut devices = Vec::new();
 let mut seen: HashSet<String> = HashSet::new();
 let entries = match std::fs::read_dir(usb_root) {
 Ok(e) => e,
 Err(_) => return devices,
 };
 for entry in entries.flatten() {
 let dir = entry.path();
 if !dir.is_dir() {
 continue;
 }
 let manufacturer = read_trimmed(&dir, "manufacturer");
 let product = read_trimmed(&dir, "product");
 if manufacturer.is_empty() && product.is_empty() {
 continue;
 }
 let vid = read_trimmed(&dir, "idVendor");
 let pid = read_trimmed(&dir, "idProduct");
 let key = format!("{}:{}", vid, pid);
 if seen.contains(&key) {
 continue;
 }
 let class = read_trimmed(&dir, "bDeviceClass");
 let combined = format!("{} {}", manufacturer, product);
 let accept = match probe_type {
 ProbeType::Scanner => is_scanner_device(&manufacturer, &product, &class),
 ProbeType::Other => matches_hint(&combined, hint),
 };
 if !accept {
 continue;
 }
 seen.insert(key.clone());
 devices.push(UsbDevice {
 id: format!("usb:{}", entry.file_name().to_string_lossy()),
 display_name: if combined.trim().is_empty() {
 "USB Device".into()
 } else {
 combined
 },
 vendor: manufacturer,
 model: product,
 serial: read_trimmed(&dir, "serial"),
 usb_id: key,
 usb_class: class,
 });
 }
 devices
}
