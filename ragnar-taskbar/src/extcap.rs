// Copyright (C) Jörg Schröder
// One-shot toplevel thumbnail capture over the ext-image-copy-capture
// protocol stack (labwc exports all three managers):
//
//   ext_foreign_toplevel_list_v1                       — window handles
//   ext_foreign_toplevel_image_capture_source_manager  — source per toplevel
//   ext_image_copy_capture_manager_v1                  — capture session
//
// Each preview request opens a fresh connection and runs the whole
// request/response pipeline synchronously: enumerate toplevels,
// pick the handle whose title matches, create source + session,
// attach a wl_shm buffer sized from the session's buffer_size event,
// capture one frame and return the pixels. Keeping this off the
// tasklist's zwlr thread means the working code path is untouched —
// the two protocols only meet through a title match on the GTK side.
use std::os::fd::AsFd;
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};

/// Raw captured frame — BGRA/X pixels ready for a GdkTexture.
pub struct Capture {
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    /// wl_shm format the buffer was captured in (xrgb8888,
    /// argb8888 or bgr888 depending on compositor support).
    pub format: wl_shm::Format,
    pub data: Vec<u8>,
}

struct Handle {
    handle: ExtForeignToplevelHandleV1,
    title: String,
    app_id: String,
}

/// Wayland-side accumulator for one capture run.
#[derive(Default)]
struct State {
    src_mgr: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    cap_mgr: Option<ExtImageCopyCaptureManagerV1>,
    shm: Option<wl_shm::WlShm>,
    handles: Vec<Handle>,
    // Session constraint events, then the frame result.
    buf_w: u32,
    buf_h: u32,
    shm_formats: Vec<wl_shm::Format>,
    constraints_done: bool,
    ready: bool,
    failed: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
            ..
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "ext_foreign_toplevel_list_v1" => {
                registry.bind::<ExtForeignToplevelListV1, _, _>(name, version, qh, ());
            }
            "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                state.src_mgr = Some(
                    registry.bind::<ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    ),
                );
            }
            "ext_image_copy_capture_manager_v1" => {
                state.cap_mgr = Some(registry.bind::<ExtImageCopyCaptureManagerV1, _, _>(
                    name,
                    version,
                    qh,
                    (),
                ));
            }
            "wl_shm" => {
                state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event_created_child(
        _opcode: u16,
        qhandle: &QueueHandle<State>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        qhandle.make_data::<ExtForeignToplevelHandleV1, ()>(())
    }

    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.handles.push(Handle {
                handle: toplevel,
                title: String::new(),
                app_id: String::new(),
            });
        }
    }
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        use ext_foreign_toplevel_handle_v1::Event as E;
        let Some(h) = state
            .handles
            .iter_mut()
            .find(|h| h.handle.id() == handle.id())
        else {
            return;
        };
        match event {
            E::Title { title } => h.title = title,
            E::AppId { app_id } => h.app_id = app_id,
            E::Closed => h.title = "\u{0}closed".into(),
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        use ext_image_copy_capture_session_v1::Event as E;
        match event {
            E::BufferSize { width, height } => {
                state.buf_w = width;
                state.buf_h = height;
            }
            E::ShmFormat {
                format: wayland_client::WEnum::Value(f),
            } => state.shm_formats.push(f),
            E::Done => state.constraints_done = true,
            E::Stopped => state.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        use ext_image_copy_capture_frame_v1::Event as E;
        match event {
            E::Ready => state.ready = true,
            E::Failed { .. } => state.failed = true,
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore ExtImageCaptureSourceV1);
delegate_noop!(State: ignore ExtForeignToplevelImageCaptureSourceManagerV1);
delegate_noop!(State: ignore ExtImageCopyCaptureManagerV1);

/// Capture one frame of the toplevel whose title matches `title`.
/// Runs synchronously — call it from a worker thread, not the GTK
/// main loop. Returns the raw pixels or `None` when the window is
/// gone, the compositor refused, or formats didn't line up.
pub fn capture(title: &str) -> Option<Capture> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let mut st = State::default();
    conn.display().get_registry(&qh, ());
    // First roundtrip: bind managers + the ext toplevel list, which
    // immediately enumerates handles on the second one.
    queue.roundtrip(&mut st).ok()?;
    queue.roundtrip(&mut st).ok()?;

    let (src_mgr, cap_mgr, shm) = match (&st.src_mgr, &st.cap_mgr, &st.shm) {
        (Some(a), Some(b), Some(c)) => (a.clone(), b.clone(), c.clone()),
        _ => return None,
    };
    let handle = st
        .handles
        .iter()
        .find(|h| h.title == title)
        .or_else(|| st.handles.iter().find(|h| h.title.contains(title)))?
        .handle
        .clone();

    let source = src_mgr.create_source(&handle, &qh, ());
    let session = cap_mgr.create_session(
        &source,
        ext_image_copy_capture_manager_v1::Options::empty(),
        &qh,
        (),
    );
    // Wait for constraints: buffer_size + at least one shm_format.
    for _ in 0..8 {
        if st.constraints_done || st.failed {
            break;
        }
        queue.roundtrip(&mut st).ok()?;
    }
    if st.failed || st.buf_w == 0 || st.buf_h == 0 {
        return None;
    }

    // Prefer a 4-byte format; labwc may only offer bgr888.
    let (format, bpp) = if st.shm_formats.contains(&wl_shm::Format::Xrgb8888) {
        (wl_shm::Format::Xrgb8888, 4i32)
    } else if st.shm_formats.contains(&wl_shm::Format::Argb8888) {
        (wl_shm::Format::Argb8888, 4i32)
    } else if st.shm_formats.contains(&wl_shm::Format::Bgr888) {
        (wl_shm::Format::Bgr888, 3i32)
    } else {
        return None;
    };
    let (w, h, stride) = (st.buf_w as i32, st.buf_h as i32, (st.buf_w as i32 * bpp));
    let file = tempfile::tempfile().ok()?;
    file.set_len((stride * h) as u64).ok()?;
    let pool = shm.create_pool(file.as_fd(), stride * h, &qh, ());
    let buf = pool.create_buffer(0, w, h, stride, format, &qh, ());

    let frame = session.create_frame(&qh, ());
    frame.attach_buffer(&buf);
    frame.capture();
    for _ in 0..16 {
        if st.ready || st.failed {
            break;
        }
        queue.roundtrip(&mut st).ok()?;
    }
    if !st.ready || st.failed {
        return None;
    }

    use std::io::{Read, Seek, SeekFrom};
    let mut data = Vec::with_capacity((stride * h) as usize);
    let mut f = file;
    f.seek(SeekFrom::Start(0)).ok()?;
    f.read_to_end(&mut data).ok()?;
    Some(Capture {
        width: w,
        height: h,
        stride,
        format,
        data,
    })
}
