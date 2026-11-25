// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — FITS (astronomical images) via cfitsio (dlopen).
//
// Reads the primary 2-D image as float and maps it through a
// min/max stretch into RGBA8 greyscale — the same surface Liera
// Eye shows for FITS files.

use super::Decoded;
use libloading::Library;
use std::ffi::{c_char, c_int, c_long, c_void};

struct LibFits {
    open: unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, *mut c_int) -> c_int,
    close: unsafe extern "C" fn(*mut c_void, *mut c_int) -> c_int,
    get_img_dim: unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int) -> c_int,
    get_img_size: unsafe extern "C" fn(*mut c_void, c_int, *mut c_long, *mut c_int) -> c_int,
    read_pix: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        *const c_long,
        c_long,
        *mut c_void,
        *mut c_void,
        *mut c_int,
        *mut c_int,
    ) -> c_int,
    _lib: Library,
}

fn lib() -> Result<&'static LibFits, String> {
    static L: std::sync::OnceLock<Result<LibFits, String>> = std::sync::OnceLock::new();
    L.get_or_init(|| unsafe {
        let lib = Library::new("libcfitsio.so.10")
            .or_else(|_| Library::new("libcfitsio.so"))
            .map_err(|e| e.to_string())?;
        Ok(LibFits {
            open: *lib.get(b"ffopen").map_err(|e| e.to_string())?,
            close: *lib.get(b"ffclos").map_err(|e| e.to_string())?,
            get_img_dim: *lib.get(b"ffgidm").map_err(|e| e.to_string())?,
            get_img_size: *lib.get(b"ffgisz").map_err(|e| e.to_string())?,
            read_pix: *lib.get(b"ffgpv").map_err(|e| e.to_string())?,
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

const READONLY: c_int = 0;
const TFLOAT: c_int = 41;

/// Decode the primary HDU image of a .fits file into RGBA8.
pub fn try_decode(path: &str) -> Result<Decoded, String> {
    let api = lib()?;
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    unsafe {
        let mut fptr: *mut c_void = std::ptr::null_mut();
        let mut status: c_int = 0;
        if (api.open)(&mut fptr, cpath.as_ptr(), READONLY, &mut status) != 0 {
            return Err(format!("ffopen status {status}"));
        }
        let result = (|| {
            let mut naxis: c_int = 0;
            (api.get_img_dim)(fptr, &mut naxis, &mut status);
            if status != 0 || naxis < 2 {
                return Err("not a 2-D image".into());
            }
            let mut axes = [0 as c_long; 3];
            (api.get_img_size)(fptr, naxis.min(3), axes.as_mut_ptr(), &mut status);
            let (w, h) = (axes[0] as u32, axes[1] as u32);
            if w == 0 || h == 0 || status != 0 {
                return Err("bad axes".into());
            }
            let npix = (w * h) as usize;
            let mut pixels = vec![0f32; npix];
            // Read row by row: fpixel[0]=1 (x start), fpixel[1]=row.
            for row in 0..h as usize {
                let fpix = [1 as c_long, (row + 1) as c_long, 1];
                (api.read_pix)(
                    fptr,
                    TFLOAT,
                    fpix.as_ptr(),
                    w as c_long,
                    std::ptr::null_mut(),
                    pixels[row * w as usize..].as_mut_ptr() as *mut c_void,
                    &mut 0 as *mut c_int,
                    &mut status,
                );
                if status != 0 {
                    return Err(format!("ffgpv status {status}"));
                }
            }
            Ok(Decoded::from_f32(w, h, &pixels, 1))
        })();
        let mut s2 = 0;
        (api.close)(fptr, &mut s2);
        result
    }
}
