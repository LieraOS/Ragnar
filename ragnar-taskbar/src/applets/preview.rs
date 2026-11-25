// Copyright (C) Jörg Schröder
// Hover thumbnail previews for task buttons (Cinnamon-style).
//
// The preview is its OWN small layer-shell surface sized to the
// thumbnail — deliberately NOT the fullscreen menu overlay: a
// fullscreen surface would steal the pointer's surface focus from
// the bar the moment it opens, firing task-button `leave`, which
// closes the preview, which re-enters the button — an endless
// flicker loop. A small surface leaves the pointer on the bar;
// the button only sees a real leave when the mouse moves away.
use crate::extcap;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

thread_local! {
    /// Generation counter — every enter/leave bumps it, so a late
    /// capture reply for a stale hover gets discarded.
    static GEN: Cell<u64> = const { Cell::new(0) };
    /// The currently open preview window, if any.
    static OPEN: RefCell<Option<gtk4::Window>> = const { RefCell::new(None) };
    /// True while the pointer is inside the preview window — lets it
    /// survive a task-button leave when the mouse moves up into it.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

const HOVER_MS: u64 = 400;
const LEAVE_MS: u64 = 250;
const MAX_W: f64 = 240.0;
const MAX_H: f64 = 160.0;

fn close() {
    // Take the window out BEFORE closing — the borrow_mut guard
    // must be dead by then, because unmapping fires leave handlers
    // that call close() again (RefCell double-borrow → panic).
    let w = OPEN.with(|o| o.borrow_mut().take());
    if let Some(w) = w {
        w.close();
    }
}

/// Task-button pointer-enter. `on_click` activates the window when
/// the preview itself gets clicked.
pub fn enter(button: &gtk4::Button, title: String, on_click: Rc<dyn Fn()>, on_close: Rc<dyn Fn()>) {
    GEN.with(|g| g.set(g.get() + 1));
    let gen = GEN.with(|g| g.get());
    let btn = button.clone();
    glib::timeout_add_local_once(Duration::from_millis(HOVER_MS), move || {
        if GEN.with(|g| g.get()) != gen {
            return;
        }
        let t = title.clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(extcap::capture(&t));
        });
        glib::spawn_future_local(async move {
            if let Ok(Some(cap)) = rx.recv().await {
                if GEN.with(|g| g.get()) == gen {
                    show(&btn, title, cap, on_click, on_close);
                }
            }
        });
    });
}

/// Task-button pointer-leave: cancel pending work; keep the preview
/// if the pointer moved into it, close it shortly otherwise.
pub fn leave() {
    GEN.with(|g| g.set(g.get() + 1));
    glib::timeout_add_local_once(Duration::from_millis(LEAVE_MS), || {
        if !INSIDE.with(|c| c.get()) {
            close();
        }
    });
}

/// Small layer surface above the taskbar at the applet's span —
/// same anchoring math as the menu overlay, but only as big as its
/// content so the bar keeps pointer focus everywhere else.
fn preview_window(
    widget_left: f64,
    widget_right: f64,
    surf_w: f64,
    bar_height: i32,
) -> gtk4::Window {
    close();
    let win = gtk4::Window::builder().decorated(false).build();
    win.add_css_class("menu-box");
    win.init_layer_shell();
    win.set_layer(Layer::Overlay);
    win.set_anchor(Edge::Bottom, true);
    // Ignore the bar's exclusive zone — otherwise labwc already puts
    // the surface above it and margin_bottom lifts it a SECOND bar
    // height too far. -1 makes bottom edge = screen bottom, so the
    // margin is the real gap above the bar (same as the calendar).
    win.set_exclusive_zone(-1);
    win.set_margin(Edge::Bottom, bar_height + 4);
    if widget_left > surf_w / 2.0 {
        win.set_anchor(Edge::Right, true);
        win.set_margin(Edge::Right, (surf_w - widget_right).max(0.0) as i32);
    } else {
        win.set_anchor(Edge::Left, true);
        win.set_margin(Edge::Left, widget_left.max(0.0) as i32);
    }
    OPEN.with(|o| *o.borrow_mut() = Some(win.clone()));
    win
}

/// Build the preview content (title + thumbnail inside one big
/// button) into its own small surface above the task button.
fn show(
    button: &gtk4::Button,
    title: String,
    cap: extcap::Capture,
    on_click: Rc<dyn Fn()>,
    on_close: Rc<dyn Fn()>,
) {
    let (data, stride) = match cap.format {
        // bgr888 → 3-byte pixels need a BGRA expansion for the texture.
        wayland_client::protocol::wl_shm::Format::Bgr888 => {
            let px = cap.width as usize * cap.height as usize;
            let mut out = Vec::with_capacity(px * 4);
            for bgr in cap.data.chunks_exact(3) {
                out.extend_from_slice(&[bgr[0], bgr[1], bgr[2], 255]);
            }
            (out, cap.width as usize * 4)
        }
        // xrgb8888 → same 4-byte layout, alpha byte just needs setting.
        wayland_client::protocol::wl_shm::Format::Xrgb8888 => {
            let mut d = cap.data;
            for px in d.chunks_exact_mut(4) {
                px[3] = 255;
            }
            (d, cap.stride as usize)
        }
        _ => (cap.data, cap.stride as usize),
    };
    // Scale the PIXELS to thumbnail size before building the widget —
    // a Picture's natural request is the full texture size, so layout
    // alone can never shrink it (size_request is a minimum, not a cap).
    let scale = (MAX_W / cap.width as f64)
        .min(MAX_H / cap.height as f64)
        .min(1.0);
    let (tw, th) = (
        (cap.width as f64 * scale).max(1.0) as i32,
        (cap.height as f64 * scale).max(1.0) as i32,
    );
    let pb = gtk4::gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from_owned(data),
        gtk4::gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        cap.width,
        cap.height,
        stride as i32,
    );
    let thumb = pb.scale_simple(tw, th, gtk4::gdk_pixbuf::InterpType::Bilinear);
    let tex = thumb
        .as_ref()
        .map(gdk::Texture::for_pixbuf)
        .unwrap_or_else(|| gdk::Texture::for_pixbuf(&pb));
    let pic = gtk4::Picture::for_paintable(&tex);
    pic.set_can_shrink(true);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    // Title row: name left, close button right — the close acts on
    // the window itself, then dismisses the preview.
    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let lbl = gtk4::Label::new(Some(&title));
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    lbl.set_max_width_chars(30);
    lbl.set_hexpand(true);
    lbl.set_xalign(0.0);
    head.append(&lbl);
    let x = gtk4::Button::from_icon_name("window-close-symbolic");
    x.set_has_frame(false);
    x.set_tooltip_text(Some(&crate::i18n::tr("Fenster schließen")));
    x.connect_clicked(move |_| {
        on_close();
        glib::idle_add_local_once(close);
    });
    head.append(&x);
    inner.append(&head);

    // Only the thumbnail is the activation button — the close button
    // must NOT live inside it or the parent's click wins (child
    // clicks propagate up and activated the window instead of
    // closing it).
    let big = gtk4::Button::new();
    big.set_child(Some(&pic));
    big.set_has_frame(false);
    big.connect_clicked(move |_| {
        on_click();
        // Close AFTER the click event finished propagating — closing
        // mid-dispatch makes GTK deliver to finalised widgets
        // (the Gtk-CRITICAL 'GTK_IS_WIDGET' assertions).
        glib::idle_add_local_once(close);
    });
    inner.append(&big);

    // Track pointer-in-preview so a task-button leave doesn't kill
    // it while the user hovers the preview itself.
    let mot = gtk4::EventControllerMotion::new();
    mot.connect_enter(|_, _, _| INSIDE.with(|c| c.set(true)));
    mot.connect_leave(|_| {
        INSIDE.with(|c| c.set(false));
        glib::idle_add_local_once(close);
    });
    inner.add_controller(mot);

    let (lft, rgt, sw, h) = crate::menu::widget_span(button);
    let win = preview_window(lft, rgt, sw, h);
    win.set_child(Some(&inner));
    win.present();
}
