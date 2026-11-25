// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — typed error for SANE status codes.

use super::sys;
use std::ffi::CStr;
use std::fmt;

/// One failure reported by libsane, carrying the raw status code.
#[derive(Debug, Clone)]
pub struct SaneError {
 /// The SANE_STATUS_* code returned by the call.
 pub code: i32,
 /// Human readable text from sane_strstatus.
 pub message: String,
}

impl SaneError {
 /// Wrap a raw status; `sane_strstatus` supplies the message.
 pub fn new(code: i32) -> Self {
 let message = unsafe {
 let ptr = sys::sane_strstatus(code);
 if ptr.is_null() {
 format!("status {}", code)
 } else {
 CStr::from_ptr(ptr).to_string_lossy().into_owned()
 }
 };
 Self { code, message }
 }

 #[allow(dead_code)] // used by retry/backoff logic in later phases
 /// True when the caller may retry — busy, warming up or no docs
 /// yet in the feeder.
 pub fn is_transient(&self) -> bool {
 matches!(self.code,
 sys::SANE_STATUS_DEVICE_BUSY | sys::SANE_STATUS_WARMING_UP | sys::SANE_STATUS_NO_DOCS
 )
 }
}

impl fmt::Display for SaneError {
 fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
 write!(f, "{} (SANE {})", self.message, self.code)
 }
}

impl std::error::Error for SaneError {}

/// Convert a raw SANE_Status into Result<(), SaneError>.
pub fn check(status: i32) -> Result<(), SaneError> {
 if status == sys::SANE_STATUS_GOOD {
 Ok(())
 } else {
 Err(SaneError::new(status))
 }
}
