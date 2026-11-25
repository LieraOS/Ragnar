// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — DjVu via libdjvulibre (runtime dlopen).
//
// ddjvuapi is asynchronous: the document/page must reach
// DECODE_OK state through the message pump before render —
// matching what the C++ handler does with its wait loop.

use super::Decoded;
use libloading::Library;
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

#[repr(C)]
struct DdjvuRect {
    x: c_int,
    y: c_int,
    w: c_uint,
    h: c_uint,
}

struct LibDjVu {
    context_create: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    context_release: unsafe extern "C" fn(*mut c_void),
    cache_set_size: unsafe extern "C" fn(*mut c_void, c_ulong),
    message_wait: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    message_peek: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    message_pop: unsafe extern "C" fn(*mut c_void),
    doc_create: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_void,
    doc_release: unsafe extern "C" fn(*mut c_void),
    doc_decoding_done: unsafe extern "C" fn(*mut c_void) -> c_int,
    page_create: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
    page_release: unsafe extern "C" fn(*mut c_void),
    page_width: unsafe extern "C" fn(*mut c_void) -> c_int,
    page_height: unsafe extern "C" fn(*mut c_void) -> c_int,
    page_decoding_done: unsafe extern "C" fn(*mut c_void) -> c_int,
    format_create: unsafe extern "C" fn(c_int, c_uint, *const c_uint) -> *mut c_void,
    format_set_row_order: unsafe extern "C" fn(*mut c_void, c_int),
    format_set_y_direction: unsafe extern "C" fn(*mut c_void, c_int),
    format_release: unsafe extern "C" fn(*mut c_void),
    page_render: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        *const DdjvuRect,
        *const DdjvuRect,
        *const c_void,
        c_ulong,
        *mut c_char,
    ) -> c_int,
    _lib: Library,
}

fn lib() -> Result<&'static LibDjVu, String> {
    static L: std::sync::OnceLock<Result<LibDjVu, String>> = std::sync::OnceLock::new();
    L.get_or_init(|| unsafe {
        let lib = Library::new("libdjvulibre.so.21")
            .or_else(|_| Library::new("libdjvulibre.so"))
            .map_err(|e| e.to_string())?;
        macro_rules! s {
            ($n:literal) => {
                *lib.get(concat!($n, "\0").as_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        Ok(LibDjVu {
            context_create: s!("ddjvu_context_create"),
            context_release: s!("ddjvu_context_release"),
            cache_set_size: s!("ddjvu_cache_set_size"),
            message_wait: s!("ddjvu_message_wait"),
            message_peek: s!("ddjvu_message_peek"),
            message_pop: s!("ddjvu_message_pop"),
            doc_create: s!("ddjvu_document_create"),
            doc_release: s!("ddjvu_document_release"),
            doc_decoding_done: s!("ddjvu_document_decoding_done"),
            page_create: s!("ddjvu_page_create_by_pageno"),
            page_release: s!("ddjvu_page_release"),
            page_width: s!("ddjvu_page_get_width"),
            page_height: s!("ddjvu_page_get_height"),
            page_decoding_done: s!("ddjvu_page_decoding_done"),
            format_create: s!("ddjvu_format_create"),
            format_set_row_order: s!("ddjvu_format_set_row_order"),
            format_set_y_direction: s!("ddjvu_format_set_y_direction"),
            format_release: s!("ddjvu_format_release"),
            page_render: s!("ddjvu_page_render"),
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

const DDJVU_FORMAT_RGB24: c_int = 2;
const DDJVU_RENDER_COLOR: c_int = 0;

/// Pump messages until `done()` reports success or we hit the
/// iteration cap — ddjvuapi reports progress asynchronously.
unsafe fn pump(api: &LibDjVu, ctx: *mut c_void, done: impl Fn() -> bool) -> bool {
    for _ in 0..2000 {
        if done() {
            return true;
        }
        if !(api.message_peek)(ctx).is_null() {
            (api.message_pop)(ctx);
        } else {
            (api.message_wait)(ctx);
        }
    }
    false
}

/// Decode the first page of a .djvu document into RGBA8.
pub fn try_decode(path: &str) -> Result<Decoded, String> {
    let api = lib()?;
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let ctx = (api.context_create)(b"ragnar-eye\0".as_ptr() as *const c_char);
        if ctx.is_null() {
            return Err("ddjvu_context_create failed".into());
        }
        (api.cache_set_size)(ctx, 32 * 1024 * 1024);
        let result = (|| {
            let doc = (api.doc_create)(ctx, cpath.as_ptr(), 1);
            if doc.is_null() {
                return Err("document_create failed".into());
            }
            if !pump(api, ctx, || (api.doc_decoding_done)(doc) != 0) {
                (api.doc_release)(doc);
                return Err("document decode timed out".into());
            }
            let page = (api.page_create)(doc, 0);
            if page.is_null() {
                (api.doc_release)(doc);
                return Err("page_create failed".into());
            }
            if !pump(api, ctx, || (api.page_decoding_done)(page) != 0) {
                (api.page_release)(page);
                (api.doc_release)(doc);
                return Err("page decode timed out".into());
            }
            let w = (api.page_width)(page) as u32;
            let h = (api.page_height)(page) as u32;
            let fmt = (api.format_create)(DDJVU_FORMAT_RGB24, 0, std::ptr::null());
            (api.format_set_row_order)(fmt, 1);
            (api.format_set_y_direction)(fmt, 1);
            let rect = DdjvuRect { x: 0, y: 0, w, h };
            let mut rgb = vec![0i8; (w * h) as usize * 3];
            let ok = (api.page_render)(
                page,
                DDJVU_RENDER_COLOR,
                &rect,
                &rect,
                fmt,
                (w * 3) as c_ulong,
                rgb.as_mut_ptr() as *mut c_char,
            );
            (api.format_release)(fmt);
            (api.page_release)(page);
            (api.doc_release)(doc);
            if ok == 0 {
                return Err("page_render failed".into());
            }
            let rgb: Vec<u8> = rgb.iter().map(|b| *b as u8).collect();
            Ok(Decoded::from_rgb(w, h, &rgb))
        })();
        (api.context_release)(ctx);
        result
    }
}
