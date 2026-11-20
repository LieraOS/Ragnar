// SPDX-License-Identifier: GPL-3.0-or-later
// Wayland plumbing: connect, enumerate outputs and capture frames
// through zwlr_screencopy_manager_v1 (wlroots — labwc).
//
// wl_output.geometry carries the LOGICAL compositor position and
// wl_output.mode the pixel size, so logical size = pixels / scale.
// That is enough for both per-monitor capture and the "all
// monitors merged" composite — no xdg-output needed.

use crate::model::CapturedImage;
use std::ffi::CString;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

/// One physical output with its logical compositor rectangle.
pub struct Monitor {
    pub output: wl_output::WlOutput,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub scale: i32,
}

/// Live state shared by every Dispatch impl.
pub struct Ws {
    pub shm: Option<wl_shm::WlShm>,
    pub mgr: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    pub registry: Option<wl_registry::WlRegistry>,
    pub xdg_out: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    pub monitors: Vec<Monitor>,
    // Per-capture frame state.
    buf_info: Option<(wl_shm::Format, i32, i32, i32)>,
    buffer: Option<wl_buffer::WlBuffer>,
    mem: Option<MemfdMap>,
    ready: bool,
    failed: bool,
}

/// memfd + mmap backing store for one captured frame.
pub struct MemfdMap {
    pub ptr: *mut libc::c_void,
    pub len: usize,
}

unsafe impl Send for MemfdMap {}

impl Drop for MemfdMap {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

/// The whole wayland side bundled for the GTK app.
pub struct Wayland {
    _conn: Connection,
    queue: EventQueue<Ws>,
    state: Ws,
}

impl Wayland {
    /// Connect, bind globals, enumerate outputs.
    pub fn new() -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let mut state = Ws {
            shm: None,
            mgr: None,
            registry: None,
            xdg_out: None,
            monitors: Vec::new(),
            buf_info: None,
            buffer: None,
            mem: None,
            ready: false,
            failed: false,
        };
        // Bind the registry — its Global events fill `state` below.
        let reg: wl_registry::WlRegistry = conn.display().get_registry(&qh, ());
        state.registry = Some(reg);
        // First roundtrip collects globals, second delivers the
        // per-output events sent after binding.
        // Roundtrip 1: registry globals (order is arbitrary, so the
        // xdg-output manager may arrive after the outputs).
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        // Now bind one xdg_output per output for logical positions.
        if let Some(m) = &state.xdg_out {
            for i in 0..state.monitors.len() {
                m.get_xdg_output(&state.monitors[i].output, &qh, i);
            }
        }
        // Roundtrips 2+3: wl_output + xdg_output events.
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        if state.mgr.is_none() {
            return Err("zwlr_screencopy_manager_v1 not available".into());
        }
        Ok(Self {
            _conn: conn,
            queue,
            state,
        })
    }

    /// Human-readable monitor list for the UI.
    pub fn monitors(&self) -> &[Monitor] {
        &self.state.monitors
    }

    /// Capture every monitor and merge into one canvas using the
    /// logical compositor positions.
    pub fn capture_all(&mut self, cursor: bool) -> Result<CapturedImage, String> {
        let mut shots = Vec::new();
        for i in 0..self.state.monitors.len() {
            shots.push(self.capture_one(i, cursor)?);
        }
        let (min_x, min_y) = self
            .state
            .monitors
            .iter()
            .map(|m| (m.x, m.y))
            .fold((i32::MAX, i32::MAX), |a, b| (a.0.min(b.0), a.1.min(b.1)));
        let (max_x, max_y) = self
            .state
            .monitors
            .iter()
            .map(|m| (m.x + m.w, m.y + m.h))
            .fold((0, 0), |a, b| (a.0.max(b.0), a.1.max(b.1)));
        let (cw, ch) = ((max_x - min_x) as usize, (max_y - min_y) as usize);
        let mut canvas = vec![0u8; cw * ch * 4];
        for (i, shot) in shots.iter().enumerate() {
            let m = &self.state.monitors[i];
            // Logical → physical scale factor for this output.
            let (ox, oy) = ((m.x - min_x) as usize, (m.y - min_y) as usize);
            let fx = shot.width as f64 / m.w.max(1) as f64;
            let fy = shot.height as f64 / m.h.max(1) as f64;
            let (ox, oy) = ((ox as f64 * fx) as usize, (oy as f64 * fy) as usize);
            for row in 0..shot.height.min(ch - oy) {
                let dst = (oy + row) * cw + ox;
                let n = (shot.width).min(cw - ox) * 4;
                canvas[dst * 4..dst * 4 + n]
                    .copy_from_slice(&shot.pixels[row * shot.width * 4..row * shot.width * 4 + n]);
            }
        }
        Ok(CapturedImage {
            width: cw,
            height: ch,
            pixels: canvas,
        })
    }

    /// Capture one monitor, optionally cropped to a logical
    /// rectangle inside it (selection mode).
    pub fn capture_monitor(
        &mut self,
        idx: usize,
        rect: Option<(i32, i32, i32, i32)>,
        cursor: bool,
    ) -> Result<CapturedImage, String> {
        let full = self.capture_one(idx, cursor)?;
        let Some((rx, ry, rw, rh)) = rect else {
            return Ok(full);
        };
        let m = &self.state.monitors[idx];
        let fx = full.width as f64 / m.w.max(1) as f64;
        let fy = full.height as f64 / m.h.max(1) as f64;
        let (px, py) = ((rx as f64 * fx) as usize, (ry as f64 * fy) as usize);
        let (pw, ph) = ((rw as f64 * fx) as usize, (rh as f64 * fy) as usize);
        let (pw, ph) = (
            pw.min(full.width.saturating_sub(px)),
            ph.min(full.height.saturating_sub(py)),
        );
        let mut out = vec![0u8; pw * ph * 4];
        for row in 0..ph {
            let src = ((py + row) * full.width + px) * 4;
            out[row * pw * 4..row * pw * 4 + pw * 4]
                .copy_from_slice(&full.pixels[src..src + pw * 4]);
        }
        Ok(CapturedImage {
            width: pw,
            height: ph,
            pixels: out,
        })
    }

    /// Raw frame capture of a single output via wlr-screencopy.
    fn capture_one(&mut self, idx: usize, cursor: bool) -> Result<CapturedImage, String> {
        let qh = self.queue.handle();
        self.state.ready = false;
        self.state.failed = false;
        self.state.buf_info = None;
        let mgr = self.state.mgr.clone().ok_or("no screencopy manager")?;
        let overlay = if cursor { 1 } else { 0 };
        let frame = mgr.capture_output(overlay, &self.state.monitors[idx].output, &qh, ());
        let mut guard = 0;
        while !self.state.ready && !self.state.failed && guard < 200 {
            self.queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| e.to_string())?;
            guard += 1;
        }
        drop(frame);
        if self.state.failed {
            return Err("screencopy failed".into());
        }
        let mem = self.state.mem.take().ok_or("no buffer")?;
        let (fmt, w, h, stride) = self.state.buf_info.ok_or("no buffer info")?;
        let raw = unsafe { std::slice::from_raw_parts(mem.ptr as *const u8, mem.len) };
        convert_frame(fmt, raw, w, h, stride)
    }
}

/// memfd backed mmap sized `len` — the shm pool writes pixels here.
pub fn memfd(len: usize) -> Result<(OwnedFd, MemfdMap), String> {
    unsafe {
        let name = CString::new("ragnar-screencopy").unwrap();
        let fd = libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC);
        if fd < 0 || libc::ftruncate(fd, len as i64) < 0 {
            return Err("memfd failed".into());
        }
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        );
        if ptr == libc::MAP_FAILED {
            return Err("mmap failed".into());
        }
        Ok((OwnedFd::from_raw_fd(fd), MemfdMap { ptr, len }))
    }
}

// wl_shm advertises supported formats as events right after bind —
// a noop delegate would panic on them, so swallow them for real.
impl Dispatch<wl_shm::WlShm, ()> for Ws {
    fn event(
        _: &mut Ws,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ws>,
    ) {
    }
}
// These objects send events (e.g. wl_buffer.release) — delegate_noop
// panics on any event, so each gets a real empty handler.
macro_rules! empty_dispatch {
    ($t:ty, $ud:ty) => {
        impl Dispatch<$t, $ud> for Ws {
            fn event(
                _: &mut Ws,
                _: &$t,
                _: <$t as Proxy>::Event,
                _: &$ud,
                _: &Connection,
                _: &QueueHandle<Ws>,
            ) {
            }
        }
    };
}
empty_dispatch!(wl_shm_pool::WlShmPool, ());
empty_dispatch!(wl_buffer::WlBuffer, ());
empty_dispatch!(zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ());
empty_dispatch!(zxdg_output_manager_v1::ZxdgOutputManagerV1, ());

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, usize> for Ws {
    fn event(
        state: &mut Ws,
        _: &zxdg_output_v1::ZxdgOutputV1,
        ev: zxdg_output_v1::Event,
        idx: &usize,
        _: &Connection,
        _: &QueueHandle<Ws>,
    ) {
        let Some(m) = state.monitors.get_mut(*idx) else {
            return;
        };
        use zxdg_output_v1::Event as E;
        match ev {
            E::LogicalPosition { x, y } => {
                m.x = x;
                m.y = y;
            }
            E::LogicalSize { width, height } => {
                // Logical size is authoritative for screencopy crop.
                m.w = width;
                m.h = height;
            }
            E::Name { name } => m.name = name,
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Ws {
    fn event(
        state: &mut Ws,
        reg: &wl_registry::WlRegistry,
        ev: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Ws>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = ev
        else {
            return;
        };
        // Bind each advertised object exactly once, keyed by its
        // registry name — GlobalList::bind would return the FIRST
        // matching global again for duplicate interfaces.
        match interface.as_str() {
            "wl_shm" => {
                state.shm = Some(reg.bind::<wl_shm::WlShm, _, _>(name, version, qh, ()));
            }
            "zwlr_screencopy_manager_v1" => {
                state.mgr = Some(reg.bind(name, version.min(3), qh, ()));
            }
            "zxdg_output_manager_v1" => {
                state.xdg_out = Some(reg.bind(name, version, qh, ()));
            }
            "wl_output" => {
                let idx = state.monitors.len();
                let out: wl_output::WlOutput = reg.bind(name, version.min(4), qh, idx);
                state.monitors.push(Monitor {
                    output: out,
                    name: String::new(),
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                    scale: 1,
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, usize> for Ws {
    fn event(
        state: &mut Ws,
        _: &wl_output::WlOutput,
        ev: wl_output::Event,
        idx: &usize,
        _: &Connection,
        _: &QueueHandle<Ws>,
    ) {
        let Some(m) = state.monitors.get_mut(*idx) else {
            return;
        };
        match ev {
            wl_output::Event::Name { name } => m.name = name,
            wl_output::Event::Geometry { x, y, .. } => {
                m.x = x;
                m.y = y;
            }
            wl_output::Event::Mode {
                width,
                height,
                flags,
                ..
            } => {
                if matches!(
                    flags,
                    wayland_client::WEnum::Value(
                        wayland_client::protocol::wl_output::Mode::Current,
                    )
                ) || m.w == 0
                {
                    m.w = width;
                    m.h = height;
                }
            }
            wl_output::Event::Scale { factor } => m.scale = factor.max(1),
            wl_output::Event::Done => {
                if m.scale > 1 {
                    m.w /= m.scale;
                    m.h /= m.scale;
                }
                if m.name.is_empty() {
                    m.name = format!("Output {}", *idx + 1);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for Ws {
    fn event(
        state: &mut Ws,
        frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        ev: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Ws>,
    ) {
        use zwlr_screencopy_frame_v1::Event as E;
        match ev {
            E::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                eprintln!("[ragnar-screenshot] screencopy buffer: {format:?} {width}x{height} stride={stride}");
                let len = (stride as usize) * (height as usize);
                let Ok((fd, mem)) = memfd(len) else {
                    state.failed = true;
                    return;
                };
                let shm = match &state.shm {
                    Some(s) => s.clone(),
                    None => {
                        state.failed = true;
                        return;
                    }
                };
                let pool = shm.create_pool(fd.as_fd(), len as i32, qh, ());
                let fmt = match format {
                    wayland_client::WEnum::Value(v) => v,
                    _ => {
                        state.failed = true;
                        return;
                    }
                };
                let (w_, h_, s_) = (width as i32, height as i32, stride as i32);
                let buf = pool.create_buffer(0, w_, h_, s_, fmt, qh, ());
                state.buf_info = Some((fmt, w_, h_, s_));
                state.mem = Some(mem);
                state.buffer = Some(buf.clone());
                frame.copy(&buf);
            }
            E::LinuxDmabuf { .. } | E::Damage { .. } => {}
            E::Flags { .. } => {}
            E::Ready { .. } => {
                state.ready = true;
                frame.destroy();
            }
            E::Failed => state.failed = true,
            E::BufferDone => {}
            _ => {}
        }
    }
}

/// Bytes per pixel for the shm formats we accept.
pub fn format_bpp(fmt: wl_shm::Format) -> Option<usize> {
    use wl_shm::Format as F;
    match fmt {
        F::Argb8888 | F::Xrgb8888 | F::Abgr8888 | F::Xbgr8888 => Some(4),
        F::Bgra8888 | F::Bgrx8888 | F::Rgba8888 | F::Rgbx8888 => Some(4),
        F::Bgr888 | F::Rgb888 => Some(3),
        F::Rgb565 => Some(2),
        _ => None,
    }
}

/// Convert a raw shm frame to tightly-packed BGRA for MemoryTexture.
/// Shared by the screencopy path and the ext window capture.
pub fn convert_frame(
    fmt: wl_shm::Format,
    raw: &[u8],
    w: i32,
    h: i32,
    stride: i32,
) -> Result<CapturedImage, String> {
    use wl_shm::Format as F;
    let bpp = format_bpp(fmt).ok_or_else(|| format!("unsupported shm format {fmt:?}"))?;
    let (w, h, stride) = (w as usize, h as usize, stride as usize);
    if stride < w * bpp {
        return Err("stride smaller than row".into());
    }
    // wl_shm fourccs name the packed value MSB→LSB, NOT the memory
    // byte order. On little-endian that flips the byte sequence:
    //   Argb8888 0xAARRGGBB → mem B,G,R,A   (straight BGRA)
    //   Abgr8888 0xAABBGGRR → mem R,G,B,A   (swap R/B)
    //   Bgra8888 0xBBGGRRAA → mem A,R,G,B   (shift + swap)
    //   Rgba8888 0xRRGGBBAA → mem A,B,G,R   (shift)
    //   Rgb888   0xRRGGBB   → mem B,G,R     (straight!)
    //   Bgr888   0xBBGGRR   → mem R,G,B     (swap!)
    // Copying bytes verbatim swaps red/blue whenever the compositor
    // advertises a non-Argb format — the classic "wrong colors" bug.
    let mut px = vec![0u8; w * h * 4];
    for row in 0..h {
        for col in 0..w {
            let s = row * stride + col * bpp;
            let d = (row * w + col) * 4;
            match fmt {
                F::Argb8888 | F::Xrgb8888 => {
                    px[d] = raw[s];
                    px[d + 1] = raw[s + 1];
                    px[d + 2] = raw[s + 2];
                    px[d + 3] = if fmt == F::Argb8888 { raw[s + 3] } else { 255 };
                }
                F::Abgr8888 | F::Xbgr8888 => {
                    px[d] = raw[s + 2];
                    px[d + 1] = raw[s + 1];
                    px[d + 2] = raw[s];
                    px[d + 3] = if fmt == F::Abgr8888 { raw[s + 3] } else { 255 };
                }
                F::Bgra8888 | F::Bgrx8888 => {
                    // mem A/X,R,G,B
                    px[d] = raw[s + 3];
                    px[d + 1] = raw[s + 2];
                    px[d + 2] = raw[s + 1];
                    px[d + 3] = if fmt == F::Bgra8888 { raw[s] } else { 255 };
                }
                F::Rgba8888 | F::Rgbx8888 => {
                    // mem A/X,B,G,R
                    px[d] = raw[s + 1];
                    px[d + 1] = raw[s + 2];
                    px[d + 2] = raw[s + 3];
                    px[d + 3] = if fmt == F::Rgba8888 { raw[s] } else { 255 };
                }
                F::Bgr888 => {
                    // 'BG24' = 0xBBGGRR → mem R,G,B
                    px[d] = raw[s + 2];
                    px[d + 1] = raw[s + 1];
                    px[d + 2] = raw[s];
                    px[d + 3] = 255;
                }
                F::Rgb888 => {
                    // 'RG24' = 0xRRGGBB → mem B,G,R
                    px[d] = raw[s];
                    px[d + 1] = raw[s + 1];
                    px[d + 2] = raw[s + 2];
                    px[d + 3] = 255;
                }
                F::Rgb565 => {
                    let v = u16::from_le_bytes([raw[s], raw[s + 1]]);
                    let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
                    px[d] = ((b * 255) / 31) as u8;
                    px[d + 1] = ((g * 255) / 63) as u8;
                    px[d + 2] = ((r * 255) / 31) as u8;
                    px[d + 3] = 255;
                }
                _ => return Err(format!("unsupported shm format {fmt:?}")),
            }
        }
    }
    Ok(CapturedImage {
        width: w,
        height: h,
        pixels: px,
    })
}
