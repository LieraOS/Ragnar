// SPDX-License-Identifier: GPL-3.0-or-later
// Per-window capture: list toplevels through
// ext_foreign_toplevel_list_v1 and shoot one through
// ext_image_copy_capture_v1 (both are compositor-agnostic
// ext protocols, advertised by labwc).

use std::os::fd::AsFd;
use wayland_client::protocol::{wl_buffer, wl_registry, wl_seat, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1, ext_image_capture_source_v1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1, ext_image_copy_capture_manager_v1,
    ext_image_copy_capture_session_v1,
};

use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1, zwlr_foreign_toplevel_manager_v1,
};

use crate::model::CapturedImage;
use crate::wayland::{convert_frame, format_bpp, memfd, MemfdMap};

/// One toplevel from the ext list.
pub struct Tl {
    pub handle: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    pub title: String,
    pub app: String,
}

/// One toplevel from the zwlr list — carries the activated state.
struct WlTl {
    handle: zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    title: String,
    app: String,
    activated: bool,
    closed: bool,
}

/// Shared state.
pub struct Ts {
    shm: Option<wl_shm::WlShm>,
    ext_list: Option<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1>,
    src_mgr: Option<
        ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    >,
    cap_mgr: Option<ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1>,
    wlr_mgr: Option<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1>,
    pub tls: Vec<Tl>,
    wl_tls: Vec<WlTl>,
    // session state
    buf_w: i32,
    buf_h: i32,
    buf_fmt: Option<wl_shm::Format>,
    mem: Option<MemfdMap>,
    ready: bool,
    failed: bool,
}

/// Window list + capturer, own wayland connection.
pub struct WindowPick {
    queue: EventQueue<Ts>,
    state: Ts,
}

impl WindowPick {
    pub fn new() -> Result<Self, String> {
        let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let mut state = Ts {
            shm: None,
            ext_list: None,
            src_mgr: None,
            cap_mgr: None,
            wlr_mgr: None,
            tls: Vec::new(),
            wl_tls: Vec::new(),
            buf_w: 0,
            buf_h: 0,
            buf_fmt: None,
            mem: None,
            ready: false,
            failed: false,
        };
        let _reg = conn.display().get_registry(&qh, ());
        for _ in 0..3 {
            queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        }
        let s = Self { queue, state };
        Ok(s)
    }

    /// Wait until a toplevel becomes activated (the user clicks it),
    /// then capture it. `timeout_ms` bounds the wait; None → wait for
    /// the first activation event.
    pub fn capture_activated(&mut self, timeout_ms: u64) -> Result<CapturedImage, String> {
        let start = std::time::Instant::now();
        let idx = loop {
            if let Some(i) = self
                .state
                .wl_tls
                .iter()
                .position(|t| t.activated && !t.closed)
            {
                break i;
            }
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return Err("no window activated in time".into());
            }
            self.queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| e.to_string())?;
        };
        let (title, app) = {
            let t = &self.state.wl_tls[idx];
            (t.title.clone(), t.app.clone())
        };
        let ext_idx = self
            .state
            .tls
            .iter()
            .position(|t| t.title == title && t.app == app)
            .ok_or_else(|| format!("window '{title}' missing in ext list"))?;
        self.capture(ext_idx)
    }

    /// Capture toplevel `idx` via ext-image-copy-capture.
    pub fn capture(&mut self, idx: usize) -> Result<CapturedImage, String> {
        let qh = self.queue.handle();
        let src: ext_image_capture_source_v1::ExtImageCaptureSourceV1 = self
            .state
            .src_mgr
            .as_ref()
            .ok_or("no toplevel source manager")?
            .create_source(&self.state.tls[idx].handle, &qh, ());
        let mgr = self.state.cap_mgr.clone().ok_or("no capture manager")?;
        let session = mgr.create_session(
            &src,
            ext_image_copy_capture_manager_v1::Options::empty(),
            &qh,
            (),
        );
        // Wait until the session advertises size + shm format.
        let mut guard = 0;
        while self.state.buf_fmt.is_none() && guard < 50 {
            self.queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| e.to_string())?;
            guard += 1;
        }
        let fmt = self.state.buf_fmt.ok_or("session gave no shm format")?;
        let (w, h) = (self.state.buf_w, self.state.buf_h);
        // stride must match the advertised format's bytes-per-pixel —
        // the compositor rejects anything else with "Invalid stride".
        let bpp = format_bpp(fmt).ok_or("unsupported shm format")? as i32;
        let stride = w * bpp;
        let (fd, mem) = memfd((stride * h) as usize)?;
        let shm = self.state.shm.clone().ok_or("no shm")?;
        let pool = shm.create_pool(fd.as_fd(), stride * h, &qh, ());
        let buf = pool.create_buffer(0, w, h, stride, fmt, &qh, ());
        self.state.mem = Some(mem);
        let frame = session.create_frame(&qh, ());
        frame.attach_buffer(&buf);
        frame.capture();
        let mut guard = 0;
        while !self.state.ready && !self.state.failed && guard < 200 {
            self.queue
                .blocking_dispatch(&mut self.state)
                .map_err(|e| e.to_string())?;
            guard += 1;
        }
        session.destroy();
        src.destroy();
        if self.state.failed {
            return Err("toplevel capture failed".into());
        }
        let mem = self.state.mem.take().ok_or("no buffer")?;
        let raw = unsafe { std::slice::from_raw_parts(mem.ptr as *const u8, mem.len) };
        convert_frame(fmt, raw, w, h, stride)
    }
}

macro_rules! noop {
    ($t:ty) => {
        impl Dispatch<$t, ()> for Ts {
            fn event(
                _: &mut Ts,
                _: &$t,
                _: <$t as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Ts>,
            ) {
            }
        }
    };
}

noop!(wl_seat::WlSeat);
noop!(wl_shm::WlShm);
noop!(wl_shm_pool::WlShmPool);
noop!(wl_buffer::WlBuffer);
noop!(ext_image_capture_source_v1::ExtImageCaptureSourceV1);
noop!(ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1);
noop!(ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1);

// Registry: bind only the globals we need.
impl Dispatch<wl_registry::WlRegistry, ()> for Ts {
    fn event(
        state: &mut Ts,
        reg: &wl_registry::WlRegistry,
        ev: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Ts>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = ev
        else {
            return;
        };
        match interface.as_str() {
            "wl_shm" => state.shm = Some(reg.bind(name, version.min(1), qh, ())),
            "ext_foreign_toplevel_list_v1" => {
                state.ext_list = Some(reg.bind(name, version.min(1), qh, ()))
            }
            "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                state.src_mgr = Some(reg.bind(name, version.min(1), qh, ()))
            }
            "ext_image_copy_capture_manager_v1" => {
                state.cap_mgr = Some(reg.bind(name, version.min(1), qh, ()))
            }
            "zwlr_foreign_toplevel_manager_v1" => {
                state.wlr_mgr = Some(reg.bind(name, version.min(3), qh, ()))
            }
            _ => {}
        }
    }
}

// ext list announces each open window as a child handle.
impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, ()> for Ts {
    fn event(
        state: &mut Ts,
        _: &ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        ev: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = ev {
            state.tls.push(Tl {
                handle: toplevel,
                title: String::new(),
                app: String::new(),
            });
        }
    }

    // The `toplevel` event (opcode 0) creates handle children —
    // wayland-client needs their udata declared up front.
    wayland_client::event_created_child!(
        Ts,
        ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        [0 => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())]
    );
}

// Per-handle identity events — matched back by object id.
impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for Ts {
    fn event(
        state: &mut Ts,
        h: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        ev: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
        use ext_foreign_toplevel_handle_v1::Event as E;
        if let Some(t) = state.tls.iter_mut().find(|t| t.handle.id() == h.id()) {
            match ev {
                E::Title { title } => t.title = title,
                E::AppId { app_id } => t.app = app_id,
                _ => {}
            }
        }
    }
}

// Session advertises geometry + accepted formats.
impl Dispatch<ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1, ()> for Ts {
    fn event(
        state: &mut Ts,
        _: &ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
        ev: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
        use ext_image_copy_capture_session_v1::Event as E;
        match ev {
            E::BufferSize { width, height } => {
                state.buf_w = width as i32;
                state.buf_h = height as i32;
            }
            // Prefer a known 4-byte format, otherwise take the first.
            E::ShmFormat {
                format: wayland_client::WEnum::Value(f),
            } if state.buf_fmt.is_none()
                || matches!(f, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) =>
            {
                state.buf_fmt = Some(f);
            }
            _ => {}
        }
    }
}

// Frame: ready/failed is all that matters — pixels are in shm.
impl Dispatch<ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1, ()> for Ts {
    fn event(
        state: &mut Ts,
        _: &ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
        ev: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
        use ext_image_copy_capture_frame_v1::Event as E;
        match ev {
            E::Ready => state.ready = true,
            E::Failed { .. } => state.failed = true,
            _ => {}
        }
    }
}

// zwlr manager announces toplevels as children (event opcode 0).
impl Dispatch<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, ()> for Ts {
    fn event(
        _: &mut Ts,
        _: &zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        _: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
    }

    wayland_client::event_created_child!(
        Ts,
        zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        [0 => (zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ())]
    );
}

// Per-handle identity + activated state — matched back by object id.
impl Dispatch<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ()> for Ts {
    fn event(
        state: &mut Ts,
        h: &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        ev: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Ts>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event as E;
        if !state.wl_tls.iter().any(|t| t.handle.id() == h.id()) {
            state.wl_tls.push(WlTl {
                handle: h.clone(),
                title: String::new(),
                app: String::new(),
                activated: false,
                closed: false,
            });
        }
        let Some(t) = state.wl_tls.iter_mut().find(|t| t.handle.id() == h.id()) else {
            return;
        };
        match ev {
            E::Title { title } => t.title = title,
            E::AppId { app_id } => t.app = app_id,
            E::State { state: flags } => {
                // The array carries raw enum values — Activated is
                // what a toplevel reports once the user clicks it.
                t.activated =
                    flags.contains(&(zwlr_foreign_toplevel_handle_v1::State::Activated as u8));
            }
            E::Closed => t.closed = true,
            _ => {}
        }
    }
}
