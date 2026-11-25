// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — per-device capability probing, SANE descriptor walk.
//
// After sane_open we walk the option descriptors and derive:
// - supported DPI values (resolution/x-resolution/y-resolution)
// - scan modes (mode string list)
// - ADF / duplex availability (source string list)

use super::{option, status, sys};

/// Well-known option names from (SANE_NAME_*).
const OPT_RESOLUTION: &str = "resolution";
const OPT_MODE: &str = "mode";
const OPT_SOURCE: &str = "source";

/// Fallback DPI ladder used when a range has too many steps to
/// enumerate individually — identical to standard list.
const STANDARD_DPI: &[i32] = &[75, 100, 150, 200, 240, 300, 400, 600, 1200, 2400, 4800];

/// Probed capabilities of one opened device.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
 /// Supported DPI values, ascending, deduplicated.
 pub dpi_list: Vec<i32>,
 /// Scan modes as the backend names them, e.g. "Color", "Gray".
 pub modes: Vec<String>,
 /// Source strings offered by the backend.
 pub sources: Vec<String>,
 /// True when the source list mentions an ADF.
 pub has_adf: bool,
 /// True when a source string suggests duplex scanning.
 pub has_duplex: bool,
 /// Maximum reported DPI (range max or list max).
 pub max_dpi: i32,
}

/// Convert a constraint word to dpi: fixed-point values get
/// SANE_UNFIX applied, plain ints pass through.
fn constraint_word_to_i32(desc: &option::OptionDesc, w: i32) -> i32 {
 if desc.value_type == sys::SANE_TYPE_FIXED {
 sys::sane_unfix(w) as i32
 } else {
 w
 }
}

/// DPI values from a single resolution descriptor.
fn dpi_from_descriptor(desc: &option::OptionDesc) -> Vec<i32> {
 let mut out = Vec::new();
 if let Some((min, max, quant)) = desc.range {
 let min = constraint_word_to_i32(desc, min);
 let max = constraint_word_to_i32(desc, max);
 let quant = if quant == 0 { 1 } else { constraint_word_to_i32(desc, quant).max(1) };
 let steps = if quant > 0 { (max - min) / quant } else { 0 };
 if (0..=20).contains(&steps) {
 // Fine-grained range: enumerate every legal value.
 let mut v = min;
 while v <= max {
 out.push(v);
 v += quant;
 }
 } else {
 // Coarse range: offer the standard ladder inside the
 // range plus the maximum, like the design.
 out.extend(STANDARD_DPI.iter().copied().filter(|d| *d >= min && *d <= max));
 if !out.contains(&max) {
 out.push(max);
 }
 }
 } else if !desc.word_list.is_empty() {
 out.extend(desc.word_list.iter().map(|w| constraint_word_to_i32(desc, *w)));
 }
 out
}

/// Probe all capabilities of an open handle — walks descriptors
/// 1.count() once, so string lists are collected in one pass.
///
/// # Safety
/// `handle` must be an open SANE_Handle; calls run single-threaded
/// per device.
pub unsafe fn probe(handle: sys::SANE_Handle) -> Result<Capabilities, status::SaneError> {
 let count = option::option_count(handle)?;
 let mut caps = Capabilities::default();
 for i in 1..count {
 let desc = match option::describe(handle, i) {
 Some(d) => d,
 None => continue,
 };
 match desc.name.as_str() {
 OPT_RESOLUTION | "x-resolution" | "y-resolution" => {
 for d in dpi_from_descriptor(&desc) {
 if d > 0 && !caps.dpi_list.contains(&d) {
 caps.dpi_list.push(d);
 }
 caps.max_dpi = caps.max_dpi.max(d);
 }
 }
 OPT_MODE => {
 if caps.modes.is_empty() && !desc.string_list.is_empty() {
 caps.modes = desc.string_list.clone();
 }
 }
 OPT_SOURCE
 if caps.sources.is_empty() && !desc.string_list.is_empty() => {
 caps.sources = desc.string_list.clone();
 for s in &desc.string_list {
 if s.contains("ADF") || s.contains("Document Feeder") {
 caps.has_adf = true;
 }
 if s.to_lowercase().contains("duplex") {
 caps.has_duplex = true;
 }
 }
 }
 _ => {}
 }
 }
 caps.dpi_list.sort_unstable();
 if caps.dpi_list.is_empty() {
 // Backend offered no resolution constraint at all — fall
 // back to the standard ladder like the design does.
 caps.dpi_list = STANDARD_DPI.to_vec();
 caps.max_dpi = *STANDARD_DPI.last().unwrap();
 }
 Ok(caps)
}
