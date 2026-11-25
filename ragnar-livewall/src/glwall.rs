// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-livewall — OpenGL wallpaper widget
//
// GTK 4.16 deprecated GskGLShader and 4.18 removed support on
// every renderer, so the procedural aurora runs in a plain
// GtkGLArea instead: one fullscreen triangle, the wallpaper as
// a texture, the aurora computed in the fragment shader.
//
// Under the GLArea sits a GtkPicture with the same image — if
// GL initialisation fails the area hides itself and the static
// picture remains. auto_render is off; the scene calls
// set_time() at its 15 fps tick, which is the only redraw
// trigger (zero work while the fullscreen watcher is paused).

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_void};
use std::rc::Rc;
use std::sync::OnceLock;

use crate::shaders::{FS_ES, FS_GL, VS_ES, VS_GL};

use gtk4::prelude::*;
use gtk4::{GLArea, glib};

/// Decoded wallpaper buffer: width, height, RGBA bytes, stride.
type PixelBuf = (i32, i32, Vec<u8>, usize);

/// Uniform locations, resolved once after program link.
#[derive(Default)]
struct Uni {
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

/// Live GL objects, all valid only inside the GLArea context.
#[derive(Default)]
struct Gl {
    prog: Cell<u32>,
    tex: Cell<u32>,
    uni: RefCell<Uni>,
    /// Effect parameters for u_stars*/u_mist* — loaded once
    /// from livewall-effects.ini; the daemon respawns us on
    /// change, so no live reload is needed here.
    fx: RefCell<crate::effects::Fx>,
    /// Wallpaper dimensions for u_res (edit-rect pixel math).
    img: Cell<(i32, i32)>,
    /// ES2 path: fullscreen-quad VBO + its attribute location.
    vbo: Cell<u32>,
    a_pos: Cell<i32>,
    /// Context dialect: true = GLES2, false = desktop GL.
    es: Cell<bool>,
    time: Cell<f32>,
    failed: Cell<bool>,
    /// Downloaded wallpaper pixels — kept for texture upload.
    /// (width, height, rgba bytes, stride)
    pixels: RefCell<Option<PixelBuf>>,
    /// Function-loader handle — must outlive the GL calls.
    _lib: RefCell<Option<libloading::Library>>,
}

/// The wall widget: GLArea (aurora) over a Picture (fallback).
pub struct GLWall {
    pub area: GLArea,
    state: Rc<Gl>,
}

/// Resolve GL entry points via eglGetProcAddress (Wayland/EGL
/// session) or glXGetProcAddressARB as the X11 fallback.
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
            // Leak the handle: function pointers live forever.
            std::mem::forget(lib);
            return Ok(());
        }
        Err("no GL loader library found".into())
    })
    .clone()
}

/// Compile one shader stage; None + log on error.
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
            let mut len = 0;
            gl::GetShaderiv(sh, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = vec![0u8; len.max(1) as usize];
            gl::GetShaderInfoLog(sh, len, std::ptr::null_mut(), buf.as_mut_ptr() as _);
            eprintln!("ragnar-livewall: GLSL error: {}", String::from_utf8_lossy(&buf));
            return None;
        }
        Some(sh)
    }
}

impl GLWall {
    /// GLArea over the same texture the fallback picture shows.
    /// `fx` carries the per-image effect parameters.
    pub fn new(texture: &gtk4::gdk::Texture, fx: crate::effects::Fx) -> Self {
        let state = Rc::new(Gl::default());
        *state.fx.borrow_mut() = fx;
        state.img.set((texture.width(), texture.height()));
        // Pull pixels via the downloader in a known RGBA format.
        let mut dl = gtk4::gdk::TextureDownloader::new(texture);
        dl.set_format(gtk4::gdk::MemoryFormat::R8g8b8a8);
        let (bytes, stride) = dl.download_bytes();
        *state.pixels.borrow_mut() = Some((
            texture.width(),
            texture.height(),
            bytes.to_vec(),
            stride,
        ));

        let area = GLArea::new();
        area.set_auto_render(false);

        // Realize: load GL, build program + texture; any failure
        // hides the area so the Picture underneath shows through.
        let st = state.clone();
        area.connect_realize(move |area| {
            if let Err(e) = init_gl(area, &st) {
                eprintln!("ragnar-livewall: GL init failed ({e}) — static wallpaper");
                st.failed.set(true);
                area.set_visible(false);
            }
        });
        let st = state.clone();
        area.connect_render(move |_, _| {
            if !st.failed.get() {
                draw(&st);
            }
            glib::Propagation::Stop
        });
        Self { area, state }
    }

    /// Advance the shader clock — sole redraw trigger.
    pub fn set_time(&self, t: f32) {
        self.state.time.set(t);
        if !self.state.failed.get() {
            self.area.queue_render();
        }
    }
}

/// All GL setup inside the realize handler where a context is
/// guaranteed to exist.
fn init_gl(area: &GLArea, st: &Rc<Gl>) -> Result<(), String> {
    load_gl()?;
    area.make_current();
    if let Some(e) = area.error() {
        return Err(format!("context: {e}"));
    }
    unsafe {
        // Pick the dialect the context actually speaks — GTK on
        // Wayland usually hands out an OpenGL ES context.
        let ver = std::ffi::CStr::from_ptr(gl::GetString(gl::VERSION) as *const _)
            .to_string_lossy()
            .into_owned();
        let es = ver.contains("ES");
        st.es.set(es);
        eprintln!("ragnar-livewall: GL context: {ver}");
        let (vsrc, fsrc) = if es { (VS_ES, FS_ES) } else { (VS_GL, FS_GL) };
        let (Some(vs), Some(fs)) = (
            compile(gl::VERTEX_SHADER, vsrc),
            compile(gl::FRAGMENT_SHADER, fsrc),
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
            // ES2 path: fullscreen quad VBO (two triangles).
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
            // Core-profile safety net for the gl_VertexID path.
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);
        }

        // Upload the wallpaper once.
        let (w, h, data, _stride) = st
            .pixels
            .borrow()
            .clone()
            .ok_or("no pixels")?;
        let mut tex = 0;
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as i32,
            w,
            h,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            data.as_ptr() as *const c_void,
        );
        st.tex.set(tex);
    }
    Ok(())
}

/// One frame: bind, set uniforms, draw the triangle.
fn draw(st: &Rc<Gl>) {
    let uni = st.uni.borrow();
    let fx = st.fx.borrow();
    let (iw, ih) = st.img.get();
    unsafe {
        gl::UseProgram(st.prog.get());
        gl::Uniform1f(uni.time, st.time.get());
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
        gl::Uniform1f(uni.edit, 0.0);
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, st.tex.get());
        if st.es.get() {
            gl::BindBuffer(gl::ARRAY_BUFFER, st.vbo.get());
            gl::EnableVertexAttribArray(st.a_pos.get() as u32);
            gl::VertexAttribPointer(st.a_pos.get() as u32, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::DisableVertexAttribArray(st.a_pos.get() as u32);
        } else {
            gl::DrawArrays(gl::TRIANGLES, 0, 3);
        }
    }
}
