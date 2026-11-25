// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — JPEG XL via libjxl (runtime dlopen).
//
// Streaming decoder API: subscribe to BASIC_INFO + FULL_IMAGE,
// feed the whole file, then pull an RGBA8 output buffer. Same
// libjxl — resolved at runtime, not linked.

use super::Decoded;
use libloading::Library;
use std::ffi::{c_int, c_uint, c_void};

#[repr(C)]
struct JxlBasicInfo {
    have_container: c_int,
    xsize: c_uint,
    ysize: c_uint,
    bits_per_sample: c_uint,
    exponent_bits_per_sample: c_uint,
    intensity_target: f32,
    min_nits: f32,
    relative_to_max_display: c_int,
    linear_below: f32,
    uses_original_profile: c_int,
    have_preview: c_int,
    have_animation: c_int,
    orientation: c_uint,
    num_color_channels: c_uint,
    num_extra_channels: c_uint,
    alpha_bits: c_uint,
    alpha_exponent_bits: c_uint,
    alpha_premultiplied: c_int,
    preview_xsize: c_uint,
    preview_ysize: c_uint,
    intrinsic_xsize: c_uint,
    intrinsic_ysize: c_uint,
    padding: [u8; 100],
}

#[repr(C)]
struct JxlPixelFormat {
    num_channels: c_uint,
    data_type: c_uint,  // 0 = float, 2 = uint8
    endianness: c_uint, // 0 = native
    align: usize,
}

struct LibJxl {
    create: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    subscribe: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    set_input: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int,
    close_input: unsafe extern "C" fn(*mut c_void),
    basic_info: unsafe extern "C" fn(*const c_void, *mut JxlBasicInfo) -> c_int,
    out_size: unsafe extern "C" fn(*const c_void, *const JxlPixelFormat, *mut usize) -> c_int,
    out_buffer:
        unsafe extern "C" fn(*mut c_void, *const JxlPixelFormat, *mut c_void, usize) -> c_int,
    process: unsafe extern "C" fn(*mut c_void) -> c_int,
    _lib: Library,
}

fn lib() -> Result<&'static LibJxl, String> {
    static L: std::sync::OnceLock<Result<LibJxl, String>> = std::sync::OnceLock::new();
    L.get_or_init(|| unsafe {
        let lib = Library::new("libjxl.so.0.11")
            .or_else(|_| Library::new("libjxl.so.0.10"))
            .or_else(|_| Library::new("libjxl.so"))
            .map_err(|e| e.to_string())?;
        Ok(LibJxl {
            create: *lib.get(b"JxlDecoderCreate").map_err(|e| e.to_string())?,
            destroy: *lib.get(b"JxlDecoderDestroy").map_err(|e| e.to_string())?,
            subscribe: *lib
                .get(b"JxlDecoderSubscribeEvents")
                .map_err(|e| e.to_string())?,
            set_input: *lib.get(b"JxlDecoderSetInput").map_err(|e| e.to_string())?,
            close_input: *lib
                .get(b"JxlDecoderCloseInput")
                .map_err(|e| e.to_string())?,
            basic_info: *lib
                .get(b"JxlDecoderGetBasicInfo")
                .map_err(|e| e.to_string())?,
            out_size: *lib
                .get(b"JxlDecoderImageOutBufferSize")
                .map_err(|e| e.to_string())?,
            out_buffer: *lib
                .get(b"JxlDecoderSetImageOutBuffer")
                .map_err(|e| e.to_string())?,
            process: *lib
                .get(b"JxlDecoderProcessInput")
                .map_err(|e| e.to_string())?,
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

const JXL_STATUS_ERROR: c_int = 0;
const JXL_STATUS_BASIC_INFO: c_int = 1;
const JXL_STATUS_NEED_IMAGE_OUT_BUFFER: c_int = 5;
const JXL_STATUS_FULL_IMAGE: c_int = 4;
const JXL_TYPE_UINT8: c_uint = 2;

/// Decode a .jxl file into RGBA8.
pub fn try_decode(path: &str) -> Result<Decoded, String> {
    let api = lib()?;
    let input = std::fs::read(path).map_err(|e| e.to_string())?;
    unsafe {
        let dec = (api.create)(std::ptr::null());
        if dec.is_null() {
            return Err("JxlDecoderCreate failed".into());
        }
        let result = (|| {
            // BASIC_INFO(1) | FULL_IMAGE(4) = 5
            if (api.subscribe)(dec, JXL_STATUS_BASIC_INFO | JXL_STATUS_FULL_IMAGE) != 0 {
                return Err("subscribe failed".into());
            }
            (api.set_input)(dec, input.as_ptr(), input.len());
            (api.close_input)(dec);

            let fmt = JxlPixelFormat {
                num_channels: 4,
                data_type: JXL_TYPE_UINT8,
                endianness: 0,
                align: 0,
            };
            let mut info: JxlBasicInfo = std::mem::zeroed();
            let mut out = Vec::<u8>::new();
            loop {
                match (api.process)(dec) {
                    JXL_STATUS_BASIC_INFO => {
                        if (api.basic_info)(dec, &mut info) != 0 {
                            return Err("basic_info failed".into());
                        }
                    }
                    JXL_STATUS_NEED_IMAGE_OUT_BUFFER => {
                        let mut size: usize = 0;
                        if (api.out_size)(dec, &fmt, &mut size) != 0 {
                            return Err("out_size failed".into());
                        }
                        out.resize(size, 0);
                        if (api.out_buffer)(dec, &fmt, out.as_mut_ptr() as *mut c_void, size) != 0 {
                            return Err("out_buffer failed".into());
                        }
                    }
                    JXL_STATUS_FULL_IMAGE => {
                        return Ok(Decoded {
                            width: info.xsize,
                            height: info.ysize,
                            rgba: out,
                        });
                    }
                    JXL_STATUS_ERROR => return Err("jxl decode error".into()),
                    _ => return Err("unexpected jxl status".into()),
                }
            }
        })();
        (api.destroy)(dec);
        result
    }
}
