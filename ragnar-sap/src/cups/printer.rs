// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — CUPS printer discovery + PPD capability probe,
// /capability probing.
//
// Several CUPS queues can point at the same physical device (e.g. a
// stale generic-text queue next to the real driver queue) — we keep
// only the best-scoring queue per device URI.

use super::sys;
use std::collections::{BTreeSet, HashMap};
use std::ffi::{CStr, CString};

/// A physical printer queue usable for printing.
/// Fields feed DeviceRegistry in the next phase.
#[derive(Debug, Clone)]
pub struct CupsPrinter {
 /// Queue name — the destination id for job submission.
 pub id: String,
 /// make-and-model when present, else the queue name.
 pub display_name: String,
 /// Non-zero when this is the default queue.
 pub is_default: bool,
 /// device-uri value.
 pub device_uri: String,
 /// printer-location value.
 #[allow(dead_code)]
 pub location: String,
}

/// Capabilities parsed from a queue's PPD file.
#[derive(Debug, Clone, Default)]
pub struct PrintCapabilities {
 /// Sorted unique DPI values from the Resolution option.
 pub resolutions_dpi: Vec<i32>,
 /// PageSize choices (paper formats).
 pub media_sizes: Vec<String>,
 /// PrintQuality choices.
 pub qualities: Vec<String>,
}

/// Copy a possibly-NULL C string to an owned String.
unsafe fn cstr_or_empty(ptr: *const i8) -> String {
 if ptr.is_null() {
 String::new()
 } else {
 CStr::from_ptr(ptr).to_string_lossy().into_owned()
 }
}

/// Look up `key` inside a dest's option array.
unsafe fn option_value(options: *const sys::cups_option_t, count: i32, key: &str) -> String {
 let k = CString::new(key).unwrap_or_default();
 let v = sys::cupsGetOption(k.as_ptr(), count, options as *mut _);
 cstr_or_empty(v)
}

/// Only accept physical USB/parallel/serial printers — the 
/// rejects virtual queues (pdf/fax/save) and network URIs.
fn is_physical_printer(name: &str, uri: &str) -> bool {
 let n = name.to_lowercase();
 let u = uri.to_lowercase();
 for bad in ["pdf", "virtual", "print-to-file", "fax", "save"] {
 if n.contains(bad) {
 return false;
 }
 }
 for bad in ["socket://", "lpd://", "http://", "https://", "ipp://", "ipps://", "snmp://"] {
 if u.contains(bad) {
 return false;
 }
 }
 u.contains("usb://") || u.contains("parallel://") || u.contains("serial://")
}

/// "1200x1200dpi" / "600dpi" -> dpi integer; 0 on parse failure.
fn parse_resolution_dpi(token: &str) -> i32 {
 let pos = token.find(['x', 'X']).unwrap_or(token.len());
 token[..pos].parse().unwrap_or(0)
}

/// Choice texts from a PPD option — prefers human-readable text,
/// falls back to the machine name, mirroring collectChoices.
unsafe fn collect_choices(opt: *const sys::ppd_option_t) -> Vec<String> {
 let mut out = Vec::new();
 if opt.is_null() {
 return out;
 }
 let o = &*opt;
 for i in 0..o.num_choices as isize {
 let c = &*o.choices.offset(i);
 let text = CStr::from_ptr(c.text.as_ptr()).to_string_lossy().into_owned();
 let choice = CStr::from_ptr(c.choice.as_ptr()).to_string_lossy().into_owned();
 out.push(if text.is_empty() { choice } else { text });
 }
 out
}

/// Queue ranking used for URI dedupe — scores: not-stopped
/// +2, default +4, real make-and-model +1.
unsafe fn queue_score(d: &sys::cups_dest_t) -> i32 {
 let mut s = 0;
 if d.is_default != 0 {
 s += 4;
 }
 // printer-state 5 = IPP_PRINTER_STOPPED (paused/disabled queue).
 if option_value(d.options, d.num_options, "printer-state") != "5" {
 s += 2;
 }
 let mm = option_value(d.options, d.num_options, "printer-make-and-model");
 if !mm.to_lowercase().contains("generic") {
 s += 1;
 }
 s
}

/// Enumerate physical printers — one entry per device URI.
/// Returns an empty list when CUPS reports no destinations; the USB
/// probe fallback lands with the usb module.
pub fn discover() -> Vec<CupsPrinter> {
 let mut dests: *mut sys::cups_dest_t = std::ptr::null_mut();
 let num = unsafe { sys::cupsGetDests(&mut dests) };
 if num <= 0 || dests.is_null() {
 return Vec::new();
 }
 let mut best: HashMap<String, (i32, CupsPrinter)> = HashMap::new();
 unsafe {
 for i in 0..num as isize {
 let d = &*dests.offset(i);
 let name = cstr_or_empty(d.name);
 let uri = option_value(d.options, d.num_options, "device-uri");
 if !is_physical_printer(&name, &uri) {
 continue;
 }
 let mut model = option_value(d.options, d.num_options, "printer-make-and-model");
 if model.is_empty() {
 model = name.clone();
 }
 let printer = CupsPrinter {
 id: name.clone(),
 display_name: model,
 is_default: d.is_default != 0,
 device_uri: uri.clone(),
 location: option_value(d.options, d.num_options, "printer-location"),
 };
 let score = queue_score(d);
 match best.get(&uri) {
 Some((s, _)) if *s >= score => {}
 _ => {
 best.insert(uri, (score, printer));
 }
 }
 }
 sys::cupsFreeDests(num, dests);
 }
 best.into_values().map(|(_, p)| p).collect()
}

/// Parse a queue's PPD for resolutions/media/quality — the 
/// returns None both when no PPD exists and when the probe comes
/// back completely empty.
pub fn fetch_capabilities(device_id: &str) -> Option<PrintCapabilities> {
 let name = CString::new(device_id).unwrap_or_default();
 unsafe {
 let ppd_path = sys::cupsGetPPD2(std::ptr::null_mut(), name.as_ptr());
 if ppd_path.is_null() {
 return None;
 }
 let path = CStr::from_ptr(ppd_path).to_string_lossy().into_owned();
 let cleanup = || {
 let _ = std::fs::remove_file(&path);
 };
 let ppd = sys::ppdOpenFile(ppd_path);
 if ppd.is_null() {
 cleanup();
 return None;
 }
 let mut caps = PrintCapabilities::default();
 let mut unique: BTreeSet<i32> = BTreeSet::new();
 let res_key = CString::new("Resolution").unwrap();
 let res = sys::ppdFindOption(ppd, res_key.as_ptr());
 for choice in collect_choices(res) {
 let dpi = parse_resolution_dpi(&choice);
 if dpi > 0 {
 unique.insert(dpi);
 }
 }
 caps.resolutions_dpi = unique.into_iter().collect();
 let ps_key = CString::new("PageSize").unwrap();
 caps.media_sizes = collect_choices(sys::ppdFindOption(ppd, ps_key.as_ptr()));
 let pq_key = CString::new("PrintQuality").unwrap();
 caps.qualities = collect_choices(sys::ppdFindOption(ppd, pq_key.as_ptr()));
 sys::ppdClose(ppd);
 cleanup();
 if caps.resolutions_dpi.is_empty() && caps.media_sizes.is_empty() && caps.qualities.is_empty() {
 return None;
 }
 Some(caps)
 }
}
