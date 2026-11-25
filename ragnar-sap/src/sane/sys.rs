// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — raw FFI declarations for libsane (SANE API v1).
//
// Mirrors /usr/include/sane/. Everything here is unsafe; the
// safe wrapper lives in the sibling modules. Types are declared
// exactly as the C ABI defines them so no bindgen step is needed.
#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_int, c_uchar, c_void};

pub type SANE_Byte = c_uchar;
pub type SANE_Word = c_int;
pub type SANE_Bool = SANE_Word;
pub type SANE_Int = SANE_Word;
pub type SANE_Char = c_char;
pub type SANE_String = *mut SANE_Char;
pub type SANE_String_Const = *const SANE_Char;
pub type SANE_Handle = *mut c_void;
pub type SANE_Fixed = SANE_Word;
pub type SANE_Status = c_int;
pub type SANE_Value_Type = c_int;
pub type SANE_Unit = c_int;
pub type SANE_Constraint_Type = c_int;
pub type SANE_Action = c_int;
pub type SANE_Frame = c_int;
pub type SANE_Auth_Callback = *mut c_void;

pub const SANE_FIXED_SCALE_SHIFT: i32 = 16;

/// SANE_FIX(d) — double → fixed-point word.
pub fn sane_fix(d: f64) -> SANE_Fixed {
 (d * 65536.0) as SANE_Fixed
}

/// SANE_UNFIX(w) — fixed-point word → double.
pub fn sane_unfix(w: SANE_Word) -> f64 {
 (w as f64) / 65536.0
}

pub const SANE_STATUS_GOOD: SANE_Status = 0;
pub const SANE_STATUS_UNSUPPORTED: SANE_Status = 1;
pub const SANE_STATUS_CANCELLED: SANE_Status = 2;
pub const SANE_STATUS_DEVICE_BUSY: SANE_Status = 3;
pub const SANE_STATUS_INVAL: SANE_Status = 4;
pub const SANE_STATUS_EOF: SANE_Status = 5;
pub const SANE_STATUS_JAMMED: SANE_Status = 6;
pub const SANE_STATUS_NO_DOCS: SANE_Status = 7;
pub const SANE_STATUS_COVER_OPEN: SANE_Status = 8;
pub const SANE_STATUS_IO_ERROR: SANE_Status = 9;
pub const SANE_STATUS_NO_MEM: SANE_Status = 10;
pub const SANE_STATUS_ACCESS_DENIED: SANE_Status = 11;
pub const SANE_STATUS_WARMING_UP: SANE_Status = 12;
pub const SANE_STATUS_HW_LOCKED: SANE_Status = 13;

pub const SANE_TYPE_BOOL: SANE_Value_Type = 0;
pub const SANE_TYPE_INT: SANE_Value_Type = 1;
pub const SANE_TYPE_FIXED: SANE_Value_Type = 2;
pub const SANE_TYPE_STRING: SANE_Value_Type = 3;
pub const SANE_TYPE_BUTTON: SANE_Value_Type = 4;
pub const SANE_TYPE_GROUP: SANE_Value_Type = 5;

pub const SANE_UNIT_NONE: SANE_Unit = 0;
pub const SANE_UNIT_PIXEL: SANE_Unit = 1;
pub const SANE_UNIT_BIT: SANE_Unit = 2;
pub const SANE_UNIT_MM: SANE_Unit = 3;
pub const SANE_UNIT_DPI: SANE_Unit = 4;
pub const SANE_UNIT_PERCENT: SANE_Unit = 5;
pub const SANE_UNIT_MICROSECOND: SANE_Unit = 6;

pub const SANE_CAP_SOFT_SELECT: SANE_Int = 1 << 0;
pub const SANE_CAP_HARD_SELECT: SANE_Int = 1 << 1;
pub const SANE_CAP_SOFT_DETECT: SANE_Int = 1 << 2;
pub const SANE_CAP_EMULATED: SANE_Int = 1 << 3;
pub const SANE_CAP_AUTOMATIC: SANE_Int = 1 << 4;
pub const SANE_CAP_INACTIVE: SANE_Int = 1 << 5;
pub const SANE_CAP_ADVANCED: SANE_Int = 1 << 6;

pub fn sane_option_is_active(cap: SANE_Int) -> bool {
 cap & SANE_CAP_INACTIVE == 0
}

pub fn sane_option_is_settable(cap: SANE_Int) -> bool {
 cap & SANE_CAP_SOFT_SELECT != 0
}

pub const SANE_INFO_INEXACT: SANE_Int = 1 << 0;
pub const SANE_INFO_RELOAD_OPTIONS: SANE_Int = 1 << 1;
pub const SANE_INFO_RELOAD_PARAMS: SANE_Int = 1 << 2;

pub const SANE_CONSTRAINT_NONE: SANE_Constraint_Type = 0;
pub const SANE_CONSTRAINT_RANGE: SANE_Constraint_Type = 1;
pub const SANE_CONSTRAINT_WORD_LIST: SANE_Constraint_Type = 2;
pub const SANE_CONSTRAINT_STRING_LIST: SANE_Constraint_Type = 3;

pub const SANE_ACTION_GET_VALUE: SANE_Action = 0;
pub const SANE_ACTION_SET_VALUE: SANE_Action = 1;
pub const SANE_ACTION_SET_AUTO: SANE_Action = 2;

pub const SANE_FRAME_GRAY: SANE_Frame = 0;
pub const SANE_FRAME_RGB: SANE_Frame = 1;
pub const SANE_FRAME_RED: SANE_Frame = 2;
pub const SANE_FRAME_GREEN: SANE_Frame = 3;
pub const SANE_FRAME_BLUE: SANE_Frame = 4;

/// SANE_Device — one entry of the NULL-terminated array returned by
/// sane_get_devices. All pointers are borrowed from libsane.
#[repr(C)]
pub struct SANE_Device {
 pub name: SANE_String_Const,
 pub vendor: SANE_String_Const,
 pub model: SANE_String_Const,
 pub type_: SANE_String_Const,
}

/// SANE_Range — min/max/quant constraint for numeric options.
#[repr(C)]
pub struct SANE_Range {
 pub min: SANE_Word,
 pub max: SANE_Word,
 pub quant: SANE_Word,
}

/// Constraint union inside SANE_Option_Descriptor. Interpretation is
/// driven by constraint_type; the safe layer reads the right member.
#[repr(C)]
pub union SANE_Constraint {
 /// NULL-terminated array of SANE_String_Const.
 pub string_list: *const SANE_String_Const,
 /// Word list whose first element is the element count.
 pub word_list: *const SANE_Word,
 /// Single SANE_Range.
 pub range: *const SANE_Range,
}

/// SANE_Option_Descriptor — metadata for one device option.
#[repr(C)]
pub struct SANE_Option_Descriptor {
 pub name: SANE_String_Const,
 pub title: SANE_String_Const,
 pub desc: SANE_String_Const,
 pub type_: SANE_Value_Type,
 pub unit: SANE_Unit,
 pub size: SANE_Int,
 pub cap: SANE_Int,
 pub constraint_type: SANE_Constraint_Type,
 pub constraint: SANE_Constraint,
}

/// SANE_Parameters — frame geometry reported after sane_start.
#[repr(C)]
pub struct SANE_Parameters {
 pub format: SANE_Frame,
 pub last_frame: SANE_Bool,
 pub bytes_per_line: SANE_Int,
 pub pixels_per_line: SANE_Int,
 pub lines: SANE_Int,
 pub depth: SANE_Int,
}

#[link(name = ":libsane.so.1")]
extern "C" {
 /// Initialize the library; version_code receives the API version.
 pub fn sane_init(version_code: *mut SANE_Int, authorize: SANE_Auth_Callback) -> SANE_Status;
 /// Release all resources held by the library.
 pub fn sane_exit();
 /// Return a NULL-terminated array of pointers to SANE_Device.
 /// local_only != 0 restricts to devices without network access.
 pub fn sane_get_devices(device_list: *mut *const *const SANE_Device, local_only: SANE_Bool) -> SANE_Status;
 /// Open a device by name; NULL/empty picks the first available.
 pub fn sane_open(devicename: SANE_String_Const, handle: *mut SANE_Handle) -> SANE_Status;
 /// Close a previously opened device handle.
 pub fn sane_close(handle: SANE_Handle);
 /// Return the descriptor for option n; NULL when out of range.
 /// Option 0 always holds the option count.
 pub fn sane_get_option_descriptor(handle: SANE_Handle, option: SANE_Int) -> *const SANE_Option_Descriptor;
 /// Read or write an option value. info receives SANE_INFO_* flags.
 pub fn sane_control_option(handle: SANE_Handle, option: SANE_Int, action: SANE_Action, value: *mut c_void, info: *mut SANE_Int) -> SANE_Status;
 /// Current scan parameters (valid any time, accurate after start).
 pub fn sane_get_parameters(handle: SANE_Handle, params: *mut SANE_Parameters) -> SANE_Status;
 /// Begin image acquisition.
 pub fn sane_start(handle: SANE_Handle) -> SANE_Status;
 /// Read scan data into data; len receives the bytes written.
 pub fn sane_read(handle: SANE_Handle, data: *mut SANE_Byte, max_length: SANE_Int, len: *mut SANE_Int) -> SANE_Status;
 /// Abort the current scan; data buffers may be discarded.
 pub fn sane_cancel(handle: SANE_Handle);
 /// Map a status code to a static, human-readable string.
 pub fn sane_strstatus(status: SANE_Status) -> SANE_String_Const;
}
