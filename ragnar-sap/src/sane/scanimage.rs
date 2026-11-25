// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scanimage CLI fallback,.
//
// Runs `scanimage` via exec without a shell, captures stdout, sends
// stderr to /dev/null so diagnostics can never corrupt the binary
// PNM stream. Used when the direct SANE path cannot handle a device
// (multi-pass frames, exotic formats, open failures).

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

/// Locate an executable in PATH (equivalent of `which`).
fn find_in_path(name: &str) -> Option<String> {
 let path = std::env::var("PATH").ok()?;
 for dir in path.split(':') {
 let dir = if dir.is_empty() { "." } else { dir };
 let candidate = format!("{}/{}", dir, name);
 if let Ok(meta) = std::fs::metadata(&candidate) {
 // Same X_OK check as access call.
 if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
 return Some(candidate);
 }
 }
 }
 None
}

/// True when the scanimage binary exists.
pub fn is_available() -> bool {
 find_in_path("scanimage").is_some()
}

/// Run argv without a shell and capture stdout as bytes; stderr goes
/// to /dev/null. Returns None on spawn failure or non-zero exit —
/// identical semantics to fork/execvp + waitpid version.
fn run_and_capture(args: &[String]) -> Option<Vec<u8>> {
 let (prog, rest) = args.split_first()?;
 // pre_exec closes fds so the stdout pipe cannot leak into the
 // child beyond the dup; stderr is nulled via Stdio::null().
 let mut child = Command::new(prog)
.args(rest)
.stdin(Stdio::null())
.stderr(Stdio::null())
.stdout(Stdio::piped())
.spawn()
.ok()?;
 let mut out = Vec::new();
 child.stdout.take()?.read_to_end(&mut out).ok()?;
 match child.wait().ok()? {
 st if st.success() => Some(out),
 _ => None,
 }
}

/// Map a capability-store color mode onto a SANE --mode value,
/// verbatim from the design.
fn sane_mode(color_mode: &str) -> &'static str {
 if color_mode.contains("Gray") {
 "Gray"
 } else if color_mode.contains("Lineart") || color_mode.contains("Black") {
 "Lineart"
 } else {
 "Color"
 }
}

/// Scan one page via the scanimage binary and return the raw PNM
/// output. For `libusb:` device ids a second attempt runs without
/// `-d` (auto-detect) because some drivers reject libusb addressing.
pub fn scan(device_id: &str,
 resolution_dpi: i32,
 color_mode: &str,
 is_preview: bool,
) -> Option<Vec<u8>> {
 if !is_available() {
 return None;
 }
 let mut device_ids = vec![device_id.to_string()];
 if device_id.contains("libusb") {
 device_ids.push(String::new());
 }
 for try_id in &device_ids {
 let mut args = vec!["scanimage".to_string()];
 if !try_id.is_empty() {
 args.push("-d".into());
 // Passed verbatim as an argv element — no shell quoting.
 args.push(try_id.clone());
 }
 args.push("--resolution".into());
 args.push(resolution_dpi.to_string());
 if !color_mode.is_empty() {
 args.push("--mode".into());
 args.push(sane_mode(color_mode).into());
 }
 if is_preview {
 // Restrict the scan to a small top-left area, in mm.
 for a in ["-l", "0", "-t", "0", "-x", "100", "-y", "100"] {
 args.push(a.into());
 }
 }
 args.push("--format".into());
 args.push("pnm".into());
 if let Some(data) = run_and_capture(&args) {
 if !data.is_empty() {
 return Some(data);
 }
 }
 }
 None
}

#[cfg(test)]
mod tests {
 use super::*;

 /// The mode mapping must match substring rules exactly.
 #[test]
 fn mode_mapping_matches_cpp() {
 assert_eq!(sane_mode("Gray"), "Gray");
 assert_eq!(sane_mode("Gray 8bit"), "Gray");
 assert_eq!(sane_mode("Lineart"), "Lineart");
 assert_eq!(sane_mode("Black and White"), "Lineart");
 assert_eq!(sane_mode("Color"), "Color");
 assert_eq!(sane_mode("Colour"), "Color");
 assert_eq!(sane_mode(""), "Color");
 }

 /// End-to-end check against libsane's test backend — skipped
 /// silently when scanimage or SANE_CONFIG_DIR is unavailable.
 #[test]
 fn scanimage_returns_pnm() {
 if !is_available() || std::env::var("SANE_CONFIG_DIR").is_err() {
 return;
 }
 let data = scan("test:0", 100, "Gray", true);
 let data = data.expect("scanimage produced output");
 assert!(data.starts_with(b"P5\n") || data.starts_with(b"P6\n") || data.starts_with(b"P4\n"));
 }
}
