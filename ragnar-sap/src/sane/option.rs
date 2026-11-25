// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — safe access to SANE option descriptors and values.
//
// Mirrors SaneSession::setOption behaviour: options are
// addressed by name, the value dispatch follows the descriptor type
// (INT / FIXED via SANE_FIX / BOOL / STRING).

use super::{status, sys};
use std::ffi::{c_void, CStr, CString};

/// Parsed view of one option descriptor (owned data, no lifetimes).
/// Some fields are only consumed by the capability/UI layers
/// landing in later phases.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OptionDesc {
 /// Option index inside the device's option table.
 pub index: i32,
 /// Command-line name, e.g. "resolution", "mode", "source".
 pub name: String,
 /// SANE_TYPE_* of the stored value.
 pub value_type: i32,
 /// SANE_UNIT_* of the value.
 pub unit: i32,
 /// Byte size of one value (relevant for strings).
 pub size: i32,
 /// SANE_CAP_* bitmask.
 pub cap: i32,
 /// SANE_CONSTRAINT_* kind.
 pub constraint_type: i32,
 /// String list values (SANE_CONSTRAINT_STRING_LIST).
 pub string_list: Vec<String>,
 /// Word list values (SANE_CONSTRAINT_WORD_LIST, first word = count).
 pub word_list: Vec<i32>,
 /// Range constraint (SANE_CONSTRAINT_RANGE).
 pub range: Option<(i32, i32, i32)>,
}

/// Snapshot a descriptor referenced by index; NULL → None.
/// Caller must guarantee the handle is open.
pub unsafe fn describe(handle: sys::SANE_Handle, index: i32) -> Option<OptionDesc> {
 let d = sys::sane_get_option_descriptor(handle, index);
 if d.is_null() {
 return None;
 }
 let d = &*d;
 let name = if d.name.is_null() {
 return None;
 } else {
 CStr::from_ptr(d.name).to_string_lossy().into_owned()
 };
 let mut desc = OptionDesc {
 index,
 name,
 value_type: d.type_,
 unit: d.unit,
 size: d.size,
 cap: d.cap,
 constraint_type: d.constraint_type,
 string_list: Vec::new(),
 word_list: Vec::new(),
 range: None,
 };
 match d.constraint_type {
 sys::SANE_CONSTRAINT_STRING_LIST => {
 let mut p = d.constraint.string_list;
 while !p.is_null() && !(*p).is_null() {
 desc.string_list
.push(CStr::from_ptr(*p).to_string_lossy().into_owned());
 p = p.add(1);
 }
 }
 sys::SANE_CONSTRAINT_WORD_LIST => {
 let p = d.constraint.word_list;
 if !p.is_null() {
 let count = *p as usize;
 for i in 1..=count {
 desc.word_list.push(*p.add(i));
 }
 }
 }
 sys::SANE_CONSTRAINT_RANGE => {
 let r = d.constraint.range;
 if !r.is_null() {
 desc.range = Some(((*r).min, (*r).max, (*r).quant));
 }
 }
 _ => {}
 }
 Some(desc)
}

/// Total option count (value of option 0).
pub unsafe fn option_count(handle: sys::SANE_Handle) -> Result<i32, status::SaneError> {
 let mut n: sys::SANE_Int = 0;
 let st = sys::sane_control_option(handle,
 0,
 sys::SANE_ACTION_GET_VALUE,
 &mut n as *mut _ as *mut c_void,
 std::ptr::null_mut(),
 );
 status::check(st).map(|_| n)
}

/// Find an option descriptor by name over the whole table.
pub unsafe fn find(handle: sys::SANE_Handle, name: &str) -> Option<OptionDesc> {
 let count = option_count(handle).unwrap_or(0);
 for i in 1..count {
 if let Some(d) = describe(handle, i) {
 if d.name == name {
 return Some(d);
 }
 }
 }
 None
}

/// Set one option by name; the value string is converted per the
/// descriptor type — same rules as the design.
pub unsafe fn set_by_name(handle: sys::SANE_Handle,
 name: &str,
 value: &str,
) -> Result<(), status::SaneError> {
 let desc = match find(handle, name) {
 Some(d) => d,
 None => return Err(status::SaneError::new(sys::SANE_STATUS_INVAL)),
 };
 let st = match desc.value_type {
 sys::SANE_TYPE_INT => {
 let mut v: sys::SANE_Int = value.parse().unwrap_or(0);
 sys::sane_control_option(handle,
 desc.index,
 sys::SANE_ACTION_SET_VALUE,
 &mut v as *mut _ as *mut c_void,
 std::ptr::null_mut(),
 )
 }
 sys::SANE_TYPE_FIXED => {
 // Many backends expose resolution/dpi as fixed-point; a
 // plain int write silently fails there.
 let mut v: sys::SANE_Fixed =
 sys::sane_fix(value.parse::<f64>().unwrap_or(0.0));
 sys::sane_control_option(handle,
 desc.index,
 sys::SANE_ACTION_SET_VALUE,
 &mut v as *mut _ as *mut c_void,
 std::ptr::null_mut(),
 )
 }
 sys::SANE_TYPE_BOOL => {
 let mut v: sys::SANE_Bool = if value == "true" || value == "1" { 1 } else { 0 };
 sys::sane_control_option(handle,
 desc.index,
 sys::SANE_ACTION_SET_VALUE,
 &mut v as *mut _ as *mut c_void,
 std::ptr::null_mut(),
 )
 }
 sys::SANE_TYPE_STRING => {
 let cs = CString::new(value).unwrap_or_default();
 sys::sane_control_option(handle,
 desc.index,
 sys::SANE_ACTION_SET_VALUE,
 cs.as_ptr() as *mut c_void,
 std::ptr::null_mut(),
 )
 }
 _ => sys::SANE_STATUS_UNSUPPORTED,
 };
 status::check(st)
}
