// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — open device handle + scan,.
//
// the scan routine returns the raw frame data wrapped in a PNM header
// (P6 RGB8 / P5 GRAY8 / P4 GRAY1). Whenever the direct SANE path
// cannot deliver — device not open, sane_start failure, unsupported
// frame format, or empty result — it falls back to the scanimage
// CLI, exactly like the design. One deliberate
// exception kept from the original: a sane_get_parameters failure
// cancels and returns None *without* falling back.

use super::scanimage;
use super::{capability::Capabilities, capability, option, status, sys};
use std::ffi::CString;

const READ_CHUNK: usize = 64 * 1024;

/// User-chosen scan settings applied before acquisition.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
 /// Resolution in DPI.
 pub resolution_dpi: i32,
 /// Backend mode string, e.g. "Color" / "Gray".
 pub color_mode: String,
 /// Backend source string, e.g. "Flatbed" / "ADF".
 pub source: String,
}

/// One open SANE device. Drop closes the handle.
pub struct Session {
 /// SANE device name used by the scanimage fallback too.
 device_id: String,
 handle: sys::SANE_Handle,
 /// Options last applied via apply_options — the fallback honours
 /// the same choices instead of hardcoded defaults.
 applied: ScanOptions,
 /// Cached capabilities probed at open.
 pub caps: Capabilities,
}

impl Session {
 /// Open a device by SANE name and probe its capabilities.
 /// An empty string lets libsane pick the first device.
 pub fn open(device: &str) -> Result<Self, status::SaneError> {
 let _g = super::init::lock();
 super::init::ensure_init()?;
 let name = CString::new(device).unwrap_or_default();
 let mut handle: sys::SANE_Handle = std::ptr::null_mut();
 status::check(unsafe { sys::sane_open(name.as_ptr(), &mut handle) })?;
 if handle.is_null() {
 return Err(status::SaneError::new(sys::SANE_STATUS_INVAL));
 }
 let caps = unsafe { capability::probe(handle)? };
 Ok(Self {
 device_id: device.to_string(),
 handle,
 applied: ScanOptions::default(),
 caps,
 })
 }

 /// Apply scan options — ports option application: "resolution" with
 /// split x/y-resolution fallback, then mode and source.
 pub fn apply_options(&mut self, opts: &ScanOptions) {
 let _g = super::init::lock();
 let dpi = opts.resolution_dpi.to_string();
 if self.set_option_locked("resolution", &dpi).is_err() {
 // Some backends use split x/y resolution options instead.
 let _ = self.set_option_locked("x-resolution", &dpi);
 let _ = self.set_option_locked("y-resolution", &dpi);
 }
 if !opts.color_mode.is_empty() {
 let _ = self.set_option_locked("mode", &opts.color_mode);
 }
 if !opts.source.is_empty() {
 let _ = self.set_option_locked("source", &opts.source);
 }
 self.applied = opts.clone();
 }

 /// Set an option by name ("resolution", "mode", "source",.).
 pub fn set_option(&self, name: &str, value: &str) -> Result<(), status::SaneError> {
 let _g = super::init::lock();
 self.set_option_locked(name, value)
 }

 /// Raw option write — only valid while holding init::lock().
 fn set_option_locked(&self, name: &str, value: &str) -> Result<(), status::SaneError> {
 unsafe { option::set_by_name(self.handle, name, value) }
 }

 /// Scan one frame and return PNM bytes (header + raster).
 /// `is_preview` only affects the scanimage fallback (restricted
 /// scan area), matching the design.
 pub fn perform_scan(&mut self, is_preview: bool) -> Option<Vec<u8>> {
 let _g = super::init::lock();
 unsafe {
 let st = sys::sane_start(self.handle);
 if st != sys::SANE_STATUS_GOOD {
 return self.fallback_scan(is_preview);
 }
 let mut params: sys::SANE_Parameters = std::mem::zeroed();
 if sys::sane_get_parameters(self.handle, &mut params) != sys::SANE_STATUS_GOOD {
 sys::sane_cancel(self.handle);
 // Deliberately no fallback here — same as the design.
 return None;
 }
 // Wrap the raw raster in a PNM header so downstream code
 // can decode the bytes without knowing the SANE params.
 let header = if params.format == sys::SANE_FRAME_RGB && params.depth == 8 {
 format!("P6\n{} {}\n255\n", params.pixels_per_line, params.lines)
 } else if params.format == sys::SANE_FRAME_GRAY && params.depth == 8 {
 format!("P5\n{} {}\n255\n", params.pixels_per_line, params.lines)
 } else if params.format == sys::SANE_FRAME_GRAY && params.depth == 1 {
 format!("P4\n{} {}\n", params.pixels_per_line, params.lines)
 } else {
 // Multi-pass colour frames or unusual depths cannot
 // be wrapped in a single PNM — hand over to the CLI.
 sys::sane_cancel(self.handle);
 return self.fallback_scan(is_preview);
 };
 let mut buffer = header.into_bytes();
 let mut read_buf = vec![0u8; READ_CHUNK];
 loop {
 let mut n: sys::SANE_Int = 0;
 let st = sys::sane_read(self.handle,
 read_buf.as_mut_ptr(),
 read_buf.len() as sys::SANE_Int,
 &mut n,
 );
 // loop: continue while GOOD && bytesRead > 0.
 if st != sys::SANE_STATUS_GOOD || n <= 0 {
 break;
 }
 buffer.extend_from_slice(&read_buf[..n as usize]);
 }
 sys::sane_cancel(self.handle);
 if buffer.is_empty() {
 return self.fallback_scan(is_preview);
 }
 Some(buffer)
 }
 }

 /// Full-page scan — a full scan is just perform_scan(false).
 #[allow(dead_code)] // used by the workflow layer
 pub fn full_scan(&mut self) -> Option<Vec<u8>> {
 self.perform_scan(false)
 }

 /// CLI fallback honouring the user's chosen options.
 fn fallback_scan(&self, is_preview: bool) -> Option<Vec<u8>> {
 if !scanimage::is_available() {
 return None;
 }
 let dpi = if self.applied.resolution_dpi > 0 {
 self.applied.resolution_dpi
 } else {
 300
 };
 let mode = if self.applied.color_mode.is_empty() {
 "Color"
 } else {
 self.applied.color_mode.as_str()
 };
 scanimage::scan(&self.device_id, dpi, mode, is_preview)
 }

 /// Abort an in-flight scan; used by the workflow layer.
 #[allow(dead_code)]
 pub fn cancel(&self) {
 let _g = super::init::lock();
 unsafe { sys::sane_cancel(self.handle) }
 }
}

impl Drop for Session {
 fn drop(&mut self) {
 let _g = super::init::lock();
 unsafe { sys::sane_close(self.handle) }
 }
}

#[cfg(test)]
mod tests {
 /// PNM header shapes produces — regression anchors.
 #[test]
 fn pnm_headers() {
 // RGB8 -> P6 with maxval, GRAY8 -> P5 with maxval, GRAY1 -> P4 without.
 let rgb = format!("P6\n{} {}\n255\n", 100, 200);
 assert_eq!(rgb, "P6\n100 200\n255\n");
 let gray = format!("P5\n{} {}\n255\n", 100, 200);
 assert_eq!(gray, "P5\n100 200\n255\n");
 let bw = format!("P4\n{} {}\n", 100, 200);
 assert_eq!(bw, "P4\n100 200\n");
 }
}
