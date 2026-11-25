// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — GL plumbing for the effects preview
//
// Uniform table, GL state, shader loading and the draw pass of
// the FxPreview widget (kept in fx_preview.rs). The fragment
// shader is the exact file ragnar-livewall installs under
// /usr/share/ragnar-livewall/ — a user copy under
// ~/.local/share/ragnar-livewall/ wins as dev override.

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_void};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::prelude::*;
use gtk4::{glib, GLArea};

use super::fx_preview::FxView;

/// Uniform locations, resolved once after program link.
#[derive(Default)]
pub(crate) struct Uni {
    time: i32,
    res: i32,
    aurora_rect: i32,
    aurora: i32,
    stars_rect: i32,
    stars: i32,
    mist_rect: i32,
    mist: i32,
    snow_rect: i32,
    snow: i32,
    rain_rect: i32,
    rain: i32,
    fly_rect: i32,
    fly: i32,
    cloud_rect: i32,
    cloud: i32,
    edit: i32,
}

#[derive(Default)]
pub(crate) struct Gl {
    pub(crate) prog: Cell<u32>,
    pub(crate) tex: Cell<u32>,
    pub(crate) uni: RefCell<Uni>,
    pub(crate) vbo: Cell<u32>,
    pub(crate) a_pos: Cell<i32>,
    pub(crate) es: Cell<bool>,
    pub(crate) failed: Cell<bool>,
    pub(crate) img: Cell<(i32, i32)>,
    pub(crate) pixels: RefCell<Option<Vec<u8>>>,
    pub(crate) _lib: RefCell<Option<libloading::Library>>,
}

/// Locate the shared fragment shader for a dialect.
/// `es` picks the ES file, `false` the desktop-GL file.
fn shader_file(es: bool) -> Option<String> {
    let name = if es { "effects_es.glsl" } else { "effects_gl.glsl" };
    let mut user = glib::user_data_dir();
    user.push("ragnar-livewall");
    user.push(name);
    for p in [user, PathBuf::from(format!("/usr/share/ragnar-livewall/{name}"))] {
        if let Ok(s) = std::fs::read_to_string(&p) {
            return Some(s);
        }
    }
    None
}

/// Whether the preview can work at all — the ragnar-livewall
/// package must be installed (or a dev copy placed in
/// ~/.local/share/ragnar-livewall/).
pub fn shader_available() -> bool {
    shader_file(true).is_some() || shader_file(false).is_some()
}

/// Vertex shaders — same trivial passes as the daemon.
fn vs_src(es: bool) -> &'static str {
    if es {
        "#version 100
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}"
    } else {
        "#version 130
out vec2 v_uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}"
    }
}

/// Resolve GL entry points — identical to ragnar-livewall:
/// eglGetProcAddress first, glXGetProcAddressARB as fallback.
fn load_gl() -> Result<(), String> {
    static RES: OnceLock<Result<(), String>> = OnceLock::new();
    RES.get_or_init(|| {
        type GetProc = unsafe extern "C" fn(*const i8) -> *const c_void;
        for (lib_name, sym) in [
            ("libEGL.so.1", &b"eglGetProcAddress\0"[..]),
            ("libGL.so.1", &b"glXGetProcAddressARB\0"[..]),
        ] {
            let Ok(lib) = (unsafe { libloading::Library::new(lib_name) }) else {
                continue;
            };
            let Ok(sym_ref) = (unsafe { lib.get::<GetProc>(sym) }) else {
                continue;
            };
            let get = *sym_ref;
            gl::load_with(move |name| {
                let c = CString::new(name).unwrap();
                unsafe { get(c.as_ptr()) }
            });
            std::mem::forget(lib);
            return Ok(());
        }
        Err("no GL loader".into())
    })
    .clone()
}

fn compile(ty: u32, src: &str) -> Option<u32> {
    unsafe {
        let sh = gl::CreateShader(ty);
        let c = CString::new(src).unwrap();
        let p = c.as_ptr();
        gl::ShaderSource(sh, 1, &p, std::ptr::null());
        gl::CompileShader(sh);
        let mut ok = 0;
        gl::GetShaderiv(sh, gl::COMPILE_STATUS, &mut ok);
        if ok == 0 {
            return None;
        }
        Some(sh)
    }
}

pub(crate) fn init_gl(area: &GLArea, st: &Rc<Gl>) -> Result<(), String> {
    load_gl()?;
    area.make_current();
    if let Some(e) = area.error() {
        return Err(format!("context: {e}"));
    }
    unsafe {
        let ver = std::ffi::CStr::from_ptr(gl::GetString(gl::VERSION) as *const _)
            .to_string_lossy()
            .into_owned();
        let es = ver.contains("ES");
        st.es.set(es);
        let fsrc = shader_file(es).ok_or("shader file not found")?;
        let (Some(vs), Some(fs)) = (
            compile(gl::VERTEX_SHADER, vs_src(es)),
            compile(gl::FRAGMENT_SHADER, &fsrc),
        ) else {
            return Err("shader compile".into());
        };
        let prog = gl::CreateProgram();
        gl::AttachShader(prog, vs);
        gl::AttachShader(prog, fs);
        gl::LinkProgram(prog);
        let mut ok = 0;
        gl::GetProgramiv(prog, gl::LINK_STATUS, &mut ok);
        if ok == 0 {
            return Err("shader link".into());
        }
        st.prog.set(prog);
        let loc = |n: &std::ffi::CStr| gl::GetUniformLocation(prog, n.as_ptr());
        *st.uni.borrow_mut() = Uni {
            time: loc(c"u_time"),
            res: loc(c"u_res"),
            aurora_rect: loc(c"u_aurora_rect"),
            aurora: loc(c"u_aurora"),
            stars_rect: loc(c"u_stars_rect[0]"),
            stars: loc(c"u_stars"),
            mist_rect: loc(c"u_mist_rect[0]"),
            mist: loc(c"u_mist"),
            snow_rect: loc(c"u_snow_rect[0]"),
            snow: loc(c"u_snow"),
            rain_rect: loc(c"u_rain_rect[0]"),
            rain: loc(c"u_rain"),
            fly_rect: loc(c"u_fly_rect[0]"),
            fly: loc(c"u_fly"),
            cloud_rect: loc(c"u_cloud_rect[0]"),
            cloud: loc(c"u_cloud"),
            edit: loc(c"u_edit"),
        };
        if es {
            let verts: [f32; 12] = [
                -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
            ];
            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (verts.len() * 4) as isize,
                verts.as_ptr() as *const c_void,
                gl::STATIC_DRAW,
            );
            st.vbo.set(vbo);
            st.a_pos.set(gl::GetAttribLocation(prog, c"a_pos".as_ptr()));
        } else {
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);
        }
        let (w, h) = st.img.get();
        let data = st.pixels.borrow().clone().ok_or("no pixels")?;
        let mut tex = 0;
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA as i32, w, h, 0, gl::RGBA,
            gl::UNSIGNED_BYTE, data.as_ptr() as *const c_void,
        );
        st.tex.set(tex);
    }
    Ok(())
}

pub(crate) fn draw(st: &Rc<Gl>, fx: &FxView, t: f32) {
    let uni = st.uni.borrow();
    let (iw, ih) = st.img.get();
    unsafe {
        gl::UseProgram(st.prog.get());
        gl::Uniform1f(uni.time, t);
        gl::Uniform2f(uni.res, iw as f32, ih as f32);
        gl::Uniform4f(uni.aurora_rect, fx.aurora_rect[0], fx.aurora_rect[1],
                      fx.aurora_rect[2], fx.aurora_rect[3]);
        gl::Uniform2f(uni.aurora, fx.aurora[0], fx.aurora[1]);
        gl::Uniform4fv(uni.stars_rect, 4, fx.stars_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.stars, fx.stars[0], fx.stars[1], fx.stars[2]);
        gl::Uniform4fv(uni.mist_rect, 4, fx.mist_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.mist, fx.mist[0], fx.mist[1], fx.mist[2]);
        gl::Uniform4fv(uni.snow_rect, 4, fx.snow_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.snow, fx.snow[0], fx.snow[1], fx.snow[2]);
        gl::Uniform4fv(uni.rain_rect, 4, fx.rain_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.rain, fx.rain[0], fx.rain[1], fx.rain[2]);
        gl::Uniform4fv(uni.fly_rect, 4, fx.fly_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.fly, fx.fly[0], fx.fly[1], fx.fly[2]);
        gl::Uniform4fv(uni.cloud_rect, 4, fx.cloud_rects.as_ptr() as *const f32);
        gl::Uniform3f(uni.cloud, fx.cloud[0], fx.cloud[1], fx.cloud[2]);
        gl::Uniform1f(uni.edit, fx.edit);
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, st.tex.get());
        if st.es.get() {
            gl::BindBuffer(gl::ARRAY_BUFFER, st.vbo.get());
            gl::EnableVertexAttribArray(st.a_pos.get() as u32);
            gl::VertexAttribPointer(st.a_pos.get() as u32, 2, gl::FLOAT,
                                    gl::FALSE, 0, std::ptr::null());
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::DisableVertexAttribArray(st.a_pos.get() as u32);
        } else {
            gl::DrawArrays(gl::TRIANGLES, 0, 3);
        }
    }
}
