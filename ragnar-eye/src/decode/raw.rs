// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — camera RAW via libraw, loaded at runtime.
//
// Opened with dlopen instead of linking:
// the .deb needs only the runtime package, and a system without
// libraw simply lacks RAW support instead of failing to start.
// C API surface used: init → open_buffer → dcraw_process →
// dcraw_make_mem_image → close.

use super::Decoded;
use libloading::Library;
use std::ffi::{c_int, c_uchar, c_uint, c_void};

/// libraw_processed_image_t (first fields we read).
#[repr(C)]
struct ProcessedImage {
    kind: c_int, // 1 = bitmap, 2 = jpeg thumbnail
    height: u16,
    width: u16,
    colors: u16,
    bits: u16,
    data_size: c_uint,
    // pixel data follows inline
}

struct LibRaw {
    init: unsafe extern "C" fn(c_uint) -> *mut c_void,
    open_buffer: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> c_int,
    dcraw_process: unsafe extern "C" fn(*mut c_void) -> c_int,
    make_mem: unsafe extern "C" fn(*mut c_void, *mut c_int) -> *mut ProcessedImage,
    clear_mem: unsafe extern "C" fn(*mut c_void),
    close: unsafe extern "C" fn(*mut c_void),
    _lib: Library,
}

fn lib() -> Result<&'static LibRaw, String> {
    static L: std::sync::OnceLock<Result<LibRaw, String>> = std::sync::OnceLock::new();
    L.get_or_init(|| unsafe {
        let lib = Library::new("libraw.so.25")
            .or_else(|_| Library::new("libraw.so.23"))
            .or_else(|_| Library::new("libraw.so"))
            .map_err(|e| e.to_string())?;
        // Leak the Library so Symbols stay valid for 'static.
        Ok(LibRaw {
            init: *lib.get(b"libraw_init").map_err(|e| e.to_string())?,
            open_buffer: *lib.get(b"libraw_open_buffer").map_err(|e| e.to_string())?,
            dcraw_process: *lib
                .get(b"libraw_dcraw_process")
                .map_err(|e| e.to_string())?,
            make_mem: *lib
                .get(b"libraw_dcraw_make_mem_image")
                .map_err(|e| e.to_string())?,
            clear_mem: *lib
                .get(b"libraw_dcraw_clear_mem")
                .map_err(|e| e.to_string())?,
            close: *lib.get(b"libraw_close").map_err(|e| e.to_string())?,
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

/// Decode a camera RAW file to RGBA8 (downsizes to 8-bit if the
/// raw pipeline emits 16-bit data).
pub fn try_decode(path: &str) -> Result<Decoded, String> {
    let api = lib()?;
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    unsafe {
        let ctx = (api.init)(0);
        if ctx.is_null() {
            return Err("libraw_init failed".into());
        }
        let result = (|| {
            if (api.open_buffer)(ctx, data.as_ptr() as *const c_void, data.len()) != 0 {
                return Err("libraw_open_buffer failed".into());
            }
            if (api.dcraw_process)(ctx) != 0 {
                return Err("dcraw_process failed".into());
            }
            let mut err: c_int = 0;
            let img = (api.make_mem)(ctx, &mut err);
            if img.is_null() || err != 0 {
                return Err("make_mem_image failed".into());
            }
            let out = bitmap_to_rgba(&*img);
            (api.clear_mem)(img as *mut c_void);
            out
        })();
        (api.close)(ctx);
        result
    }
}

/// Convert libraw's BITMAP result (8/16-bit, 3 or 4 channels)
/// into RGBA8.
unsafe fn bitmap_to_rgba(img: &ProcessedImage) -> Result<Decoded, String> {
    if img.kind != 1 {
        return Err("no bitmap output".into());
    }
    let w = img.width as u32;
    let h = img.height as u32;
    let n = (w * h) as usize;
    let ch = img.colors as usize;
    let data = (img as *const ProcessedImage as *const c_uchar).add(1);
    if img.bits == 16 && ch >= 3 {
        let src = std::slice::from_raw_parts(data as *const u16, n * ch);
        let mut rgba = Vec::with_capacity(n * 4);
        for px in src.chunks_exact(ch) {
            rgba.extend_from_slice(&[
                (px[0] >> 8) as u8,
                (px[1] >> 8) as u8,
                (px[2] >> 8) as u8,
                255,
            ]);
        }
        Ok(Decoded {
            width: w,
            height: h,
            rgba,
        })
    } else if img.bits == 8 && ch >= 3 {
        let src = std::slice::from_raw_parts(data, n * ch);
        let mut rgba = Vec::with_capacity(n * 4);
        for px in src.chunks_exact(ch) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
        Ok(Decoded {
            width: w,
            height: h,
            rgba,
        })
    } else {
        Err(format!(
            "unsupported raw layout: {} ch/{} bit",
            ch, img.bits
        ))
    }
}
