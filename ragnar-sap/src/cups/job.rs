// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — CUPS job submission,.

use super::sys;
use std::ffi::{CStr, CString};

/// Typed error for a failed submission — wraps cupsLastErrorString.
#[derive(Debug)]
pub struct PrintError(pub String);

impl std::fmt::Display for PrintError {
 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 write!(f, "{}", self.0)
 }
}

impl std::error::Error for PrintError {}

/// Submit an existing file to a destination queue.
/// Used by PrintWorkflow landing in a later phase.
/// `options` are name/value pairs (copies, quality,.) translated
/// into the CUPS option array. Returns the CUPS job id (> 0).
#[allow(dead_code)]
pub fn submit(destination: &str,
 file_path: &str,
 job_title: &str,
 options: &[(String, String)],
) -> Result<i32, PrintError> {
 if destination.is_empty() || file_path.is_empty() {
 return Err(PrintError("empty destination or file".into()));
 }
 unsafe {
 let mut cups_options: *mut sys::cups_option_t = std::ptr::null_mut();
 let mut num: i32 = 0;
 for (k, v) in options {
 let k = CString::new(k.as_str()).unwrap_or_default();
 let v = CString::new(v.as_str()).unwrap_or_default();
 num = sys::cupsAddOption(k.as_ptr(), v.as_ptr(), num, &mut cups_options);
 }
 let dest = CString::new(destination).unwrap_or_default();
 let path = CString::new(file_path).unwrap_or_default();
 let title = CString::new(job_title).unwrap_or_default();
 // NULL http = CUPS_HTTP_DEFAULT (default server connection).
 let job_id = sys::cupsPrintFile2(std::ptr::null_mut(),
 dest.as_ptr(),
 path.as_ptr(),
 title.as_ptr(),
 num,
 cups_options,
 );
 sys::cupsFreeOptions(num, cups_options);
 if job_id <= 0 {
 let err = sys::cupsLastErrorString();
 let msg = if err.is_null() {
 "unknown CUPS error".into()
 } else {
 CStr::from_ptr(err).to_string_lossy().into_owned()
 };
 return Err(PrintError(msg));
 }
 Ok(job_id)
 }
}
