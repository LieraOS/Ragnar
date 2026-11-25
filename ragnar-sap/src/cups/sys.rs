// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — raw FFI declarations for libcups (CUPS API v2.4).
//
// Mirrors cups/ and cups/. Everything here is unsafe;
// the safe wrapper lives in the sibling modules. The PPD API is
// officially deprecated since CUPS 1.6 but remains the mechanism
// the design uses for per-queue capability probing.
#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_float, c_int, c_void};

/// cups_option_t — one name/value pair in a dest's option array.
#[repr(C)]
pub struct cups_option_t {
 pub name: *mut c_char,
 pub value: *mut c_char,
}

/// cups_dest_t — a print queue or class as returned by cupsGetDests.
#[repr(C)]
pub struct cups_dest_t {
 /// Printer or class name.
 pub name: *mut c_char,
 /// Local instance name or NULL.
 pub instance: *mut c_char,
 /// Non-zero when this is the default printer.
 pub is_default: c_int,
 /// Number of entries in `options`.
 pub num_options: c_int,
 /// Option array (device-uri, printer-state,.).
 pub options: *mut cups_option_t,
}

/// Opaque PPD file handle.
pub type ppd_file_t = c_void;

const PPD_MAX_NAME: usize = 41;
const PPD_MAX_TEXT: usize = 81;

/// ppd_choice_t — one selectable value inside a ppd_option_t.
/// `choice`/`text` are fixed inline arrays, NOT pointers.
#[repr(C)]
pub struct ppd_choice_t {
 /// Non-zero when currently marked/selected.
 pub marked: c_char,
 /// Computer-readable option name.
 pub choice: [c_char; PPD_MAX_NAME],
 /// Human-readable option name.
 pub text: [c_char; PPD_MAX_TEXT],
 /// Code to send for this option.
 pub code: *mut c_char,
 /// Pointer to the parent option structure.
 pub option: *mut c_void,
}

/// ppd_option_t — one PPD option with its choice list.
#[repr(C)]
pub struct ppd_option_t {
 /// Non-zero when option conflicts exist.
 pub conflicted: c_char,
 /// Option keyword name ("PageSize", etc.).
 pub keyword: [c_char; PPD_MAX_NAME],
 /// Default option choice.
 pub defchoice: [c_char; PPD_MAX_NAME],
 /// Human-readable text.
 pub text: [c_char; PPD_MAX_TEXT],
 /// ppd_ui_t enum value.
 pub ui: c_int,
 /// ppd_section_t enum value.
 pub section: c_int,
 /// Order number.
 pub order: c_float,
 /// Number of choices.
 pub num_choices: c_int,
 /// Choice array.
 pub choices: *mut ppd_choice_t,
}

#[link(name = ":libcups.so.2")]
extern "C" {
 /// Enumerate all destinations; returns count, fills dests.
 /// Free with cupsFreeDests.
 pub fn cupsGetDests(dests: *mut *mut cups_dest_t) -> c_int;
 /// Release the dest array.
 pub fn cupsFreeDests(num_dests: c_int, dests: *mut cups_dest_t);
 /// Look up `name` in an option array; NULL when absent.
 pub fn cupsGetOption(name: *const c_char, num_options: c_int, options: *mut cups_option_t) -> *const c_char;
 /// Append one name/value pair; returns the new count.
 pub fn cupsAddOption(name: *const c_char, value: *const c_char, num_options: c_int, options: *mut *mut cups_option_t) -> c_int;
 /// Release an option array built by cupsAddOption.
 pub fn cupsFreeOptions(num_options: c_int, options: *mut cups_option_t);
 /// Submit an existing file to a destination. http == NULL uses
 /// the default server (CUPS_HTTP_DEFAULT). Returns job id > 0.
 pub fn cupsPrintFile2(http: *mut c_void, name: *const c_char, filename: *const c_char, title: *const c_char, num_options: c_int, options: *mut cups_option_t) -> c_int;
 /// Thread-local error text for the last failed call.
 pub fn cupsLastErrorString() -> *const c_char;
 /// Fetch a printer's PPD into a temp file; caller must unlink
 /// the returned path. NULL when no PPD exists.
 pub fn cupsGetPPD2(http: *mut c_void, name: *const c_char) -> *const c_char;

 /// Parse a PPD file; NULL on failure.
 pub fn ppdOpenFile(filename: *const c_char) -> *mut ppd_file_t;
 /// Release a PPD file.
 pub fn ppdClose(ppd: *mut ppd_file_t);
 /// Find one option by keyword; NULL when absent.
 pub fn ppdFindOption(ppd: *mut ppd_file_t, keyword: *const c_char) -> *mut ppd_option_t;
}
