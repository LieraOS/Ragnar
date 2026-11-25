// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — SANE device enumeration.
//
// Ports the discovery rules from ragnar-sap: sane_get_devices gives
// every backend-registered device; we reject cameras/webcams and
// keep scanners and multifunction devices.

use super::{init, status, sys};
use std::ffi::CStr;

/// A scanner as reported by libsane.
#[derive(Debug, Clone)]
pub struct SaneDevice {
 /// Backend device name — the id passed to sane_open.
 pub id: String,
 /// Vendor string, may be empty.
 pub vendor: String,
 /// Model name — used as display name.
 pub model: String,
 /// Backend-reported type, e.g. "flatbed scanner".
 pub device_type: String,
}

/// Copy a possibly-NULL C string into an owned String.
unsafe fn cstr_or_empty(ptr: sys::SANE_String_Const) -> String {
 if ptr.is_null() {
 String::new()
 } else {
 CStr::from_ptr(ptr).to_string_lossy().into_owned()
 }
}

/// True when the descriptor looks like a scanner or multifunction
/// device — mirrors isValidSaneDevice in backend: cameras
/// and video devices are rejected, scanner/mfp keywords accepted.
fn is_valid_sane_device(dev: &SaneDevice) -> bool {
 if dev.model.is_empty() && dev.vendor.is_empty() {
 return false;
 }
 let combined = format!("{} {} {}", dev.vendor, dev.model, dev.device_type).to_lowercase();
 if combined.contains("webcam")
 || combined.contains("camera")
 || combined.contains("usb camera")
 || combined.contains("video")
 {
 return false;
 }
 combined.contains("scanner")
 || combined.contains("mfp")
 || combined.contains("multifunction")
 || combined.contains("all-in-one")
 || combined.contains("copier")
 || combined.contains("mfc")
}

/// Enumerate every scanner libsane knows about.
/// Returns an empty list when sane_get_devices fails — callers that
/// want the raw error can check via init::ensure_init().
pub fn discover() -> Result<Vec<SaneDevice>, status::SaneError> {
 let _g = init::lock();
 init::ensure_init()?;
 let mut list: *const *const sys::SANE_Device = std::ptr::null();
 let st = unsafe { sys::sane_get_devices(&mut list, 0) };
 status::check(st)?;
 let mut out = Vec::new();
 if !list.is_null() {
 unsafe {
 let mut it = list;
 while !(*it).is_null() {
 let d = &**it;
 let dev = SaneDevice {
 id: cstr_or_empty(d.name),
 vendor: cstr_or_empty(d.vendor),
 model: cstr_or_empty(d.model),
 device_type: cstr_or_empty(d.type_),
 };
 if is_valid_sane_device(&dev) {
 out.push(dev);
 }
 it = it.add(1);
 }
 }
 }
 Ok(out)
}
