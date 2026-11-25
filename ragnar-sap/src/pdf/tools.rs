// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — external tool runners for PdfManager.
//
// The builds shell strings and calls std::system — we run the
// same tools through argv without a shell: identical behaviour, no
// quoting/injection surface.

use std::process::Command;

/// Run an argv tool; true on exit code 0 — the 's std::system
/// result check.
pub fn run(args: &[&str]) -> bool {
 if args.is_empty() {
 return false;
 }
 Command::new(args[0])
.args(&args[1..])
.stdin(std::process::Stdio::null())
.stdout(std::process::Stdio::null())
.stderr(std::process::Stdio::null())
.status()
.map(|s| s.success())
.unwrap_or(false)
}

/// Run a tool and capture stdout (for pdfinfo parsing).
pub fn capture(args: &[&str]) -> Option<String> {
 let out = Command::new(args[0])
.args(&args[1..])
.stdin(std::process::Stdio::null())
.stderr(std::process::Stdio::null())
.output()
.ok()?;
 if !out.status.success() {
 return None;
 }
 String::from_utf8(out.stdout).ok()
}
