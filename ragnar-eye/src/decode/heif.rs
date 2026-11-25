// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — HEIF/HEIC/AVIF via libheif (runtime dlopen).
//
// libheif handles both HEIC and AVIF containers through its codec
// plugins. Pure C API: context → primary image
// handle → decode to interleaved RGBA → read plane.

use super::Decoded;
use libloading::Library;
use std::ffi::{c_char, c_int, c_uchar, c_void};

struct LibHeif {
    context_alloc: unsafe extern "C" fn() -> *mut c_void,
    context_read: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_void) -> c_int,
    primary_handle: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int,
    decode_image:
        unsafe extern "C" fn(*mut c_void, *mut *mut c_void, c_int, c_int, *const c_void) -> c_int,
    get_plane: unsafe extern "C" fn(*const c_void, c_int, *mut c_int) -> *const c_uchar,
    get_width: unsafe extern "C" fn(*const c_void, c_int) -> c_int,
    get_height: unsafe extern "C" fn(*const c_void, c_int) -> c_int,
    image_release: unsafe extern "C" fn(*const c_void),
    handle_release: unsafe extern "C" fn(*const c_void),
    context_free: unsafe extern "C" fn(*mut c_void),
    _lib: Library,
}

fn lib() -> Result<&'static LibHeif, String> {
    static L: std::sync::OnceLock<Result<LibHeif, String>> = std::sync::OnceLock::new();
    L.get_or_init(|| unsafe {
        let lib = Library::new("libheif.so.1")
            .or_else(|_| Library::new("libheif.so"))
            .map_err(|e| e.to_string())?;
        Ok(LibHeif {
            context_alloc: *lib.get(b"heif_context_alloc").map_err(|e| e.to_string())?,
            context_read: *lib
                .get(b"heif_context_read_from_file")
                .map_err(|e| e.to_string())?,
            primary_handle: *lib
                .get(b"heif_context_get_primary_image_handle")
                .map_err(|e| e.to_string())?,
            decode_image: *lib.get(b"heif_decode_image").map_err(|e| e.to_string())?,
            get_plane: *lib
                .get(b"heif_image_get_plane_readonly")
                .map_err(|e| e.to_string())?,
            get_width: *lib
                .get(b"heif_image_get_width")
                .map_err(|e| e.to_string())?,
            get_height: *lib
                .get(b"heif_image_get_height")
                .map_err(|e| e.to_string())?,
            image_release: *lib.get(b"heif_image_release").map_err(|e| e.to_string())?,
            handle_release: *lib
                .get(b"heif_image_handle_release")
                .map_err(|e| e.to_string())?,
            context_free: *lib.get(b"heif_context_free").map_err(|e| e.to_string())?,
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

const HEIF_COLORSPACE_RGB: c_int = 10;
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

/// Decode a .heic/.heif/.avif file into RGBA8.
pub fn try_decode(path: &str) -> Result<Decoded, String> {
    let api = lib()?;
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let ctx = (api.context_alloc)();
        if ctx.is_null() {
            return Err("heif_context_alloc failed".into());
        }
        let result = (|| {
            if (api.context_read)(ctx, cpath.as_ptr(), std::ptr::null()) != 0 {
                return Err("heif_context_read failed".into());
            }
            let mut handle: *mut c_void = std::ptr::null_mut();
            if (api.primary_handle)(ctx, &mut handle) != 0 || handle.is_null() {
                return Err("no primary image".into());
            }
            let mut img: *mut c_void = std::ptr::null_mut();
            let r = (api.decode_image)(
                handle,
                &mut img,
                HEIF_COLORSPACE_RGB,
                HEIF_CHROMA_INTERLEAVED_RGBA,
                std::ptr::null(),
            );
            if r != 0 || img.is_null() {
                (api.handle_release)(handle);
                return Err("heif_decode_image failed".into());
            }
            let mut stride: c_int = 0;
            let plane = (api.get_plane)(img, HEIF_CHANNEL_INTERLEAVED, &mut stride);
            let w = (api.get_width)(img, HEIF_CHANNEL_INTERLEAVED) as u32;
            let h = (api.get_height)(img, HEIF_CHANNEL_INTERLEAVED) as u32;
            let out = if plane.is_null() || w == 0 || h == 0 {
                Err("empty image".into())
            } else {
                let mut rgba = vec![0u8; (w * h) as usize * 4];
                for row in 0..h as usize {
                    let src = plane.add(row * stride as usize);
                    let dst = &mut rgba[row * w as usize * 4..(row + 1) * w as usize * 4];
                    std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), dst.len());
                }
                Ok(Decoded {
                    width: w,
                    height: h,
                    rgba,
                })
            };
            (api.image_release)(img);
            (api.handle_release)(handle);
            out
        })();
        (api.context_free)(ctx);
        result
    }
}
