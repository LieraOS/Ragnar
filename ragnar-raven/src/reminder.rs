//! Reminder daemon + slide-in notification.
//!
//! A glib timer wakes every 30 s and compares every appointment's
//! VALARM offsets against the clock: when `start - offset` falls into
//! the current tick window and has not fired yet, a small overlay
//! window slides in from the right edge, stays ~18 s and slides back
//! out. Fired reminders are tracked per (uid, offset) so each VALARM
//! fires exactly once per process lifetime.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::collections::HashSet;
use std::rc::Rc;

use crate::i18n::tr;
use crate::store::Store;

/// Width of the reminder card — small, readable, upper-right area.
const CARD_WIDTH: i32 = 300;

/// Shared settings.ini accessor — the same file the taskbar menu
/// and (later) ragnar-settings write into.
fn cfg() -> glib::KeyFile {
    let kf = glib::KeyFile::new();
    let path = glib::user_config_dir()
        .join("ragnar-raven")
        .join("settings.ini");
    let _ = kf.load_from_file(&path, glib::KeyFileFlags::NONE);
    kf
}

/// How long the card stays on screen — `[notifications] duration_s`
/// in settings.ini, default 18 s. ragnar-settings will expose this.
fn hold_ms() -> u64 {
    let s = cfg()
        .integer("notifications", "duration_s")
        .unwrap_or(18);
    (s.max(2) as u64) * 1000
}

/// Optional notification sound — `[notifications] sound` holds a
/// freedesktop sound-event id (e.g. `dialog-information`, `bell`);
/// empty/absent means silent (the default). Played once per slide,
/// not per monitor.
fn play_sound() {
    let name = cfg()
        .string("notifications", "sound")
        .unwrap_or_default();
    if name.is_empty() || name == "none" {
        return;
    }
    let _ = std::process::Command::new("canberra-gtk-play")
        .arg("-i")
        .arg(name.as_str())
        .spawn();
}

thread_local! {
    /// (uid, offset-minutes) pairs that already fired this session.
    static FIRED: std::cell::RefCell<HashSet<(String, i64)>> =
        std::cell::RefCell::new(HashSet::new());
}

/// Whether the user globally enabled appointment notifications
/// (the same settings.ini the taskbar calendar menu toggles).
fn enabled() -> bool {
    cfg().boolean("notifications", "enabled").unwrap_or(true)
}

/// Start the reminder loop. Idempotent — later --day activations on
/// the running daemon must not stack extra timers.
pub fn start(store: &Store) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if ONCE.is_completed() {
        return;
    }
    ONCE.call_once(|| ());
    let store = store.clone();
    glib::timeout_add_seconds_local(30, move || {
        tick(&store);
        glib::ControlFlow::Continue
    });
}

/// One timer pass: fire every reminder whose time is inside the last
/// 30-second window and has not fired yet.
fn tick(store: &Store) {
    if !enabled() {
        return;
    }
    let Some(now) = glib::DateTime::now_local().ok() else {
        return;
    };
    for a in store.all() {
        for m in &a.reminders {
            let Some(fire_at) = a.start.add(glib::TimeSpan::from_minutes(-*m)).ok()
            else {
                continue;
            };
            // Due if the fire time lies within the last 60 s (the
            // 30 s tick may run a little late). Mark as fired only
            // inside the due branch — marking future reminders would
            // skip them forever when their time actually comes.
            let diff = now.difference(&fire_at).as_seconds();
            if diff >= 0 && diff < 60 {
                let key = (a.uid.clone(), *m);
                if FIRED.with(|f| f.borrow_mut().insert(key)) {
                    let when = a.start.format("%H:%M").unwrap();
                    slide_in(None, &tr("Termin"), &format!("{}  {}", when, a.title));
                }
            }
        }
    }
}

/// Which monitors get the card. `monitor = all` (or unset) puts one
/// card on every output; a connector name (e.g. `DP-3`) pins it to
/// that monitor. ragnar-settings will write this key later — the two
/// components stay decoupled through the shared settings.ini.
fn target_monitors() -> Vec<gtk4::gdk::Monitor> {
    let want = cfg()
        .string("notifications", "monitor")
        .unwrap_or_else(|_| "all".into());
    let disp = gtk4::gdk::Display::default().unwrap();
    let list = disp.monitors();
    let mut out = Vec::new();
    for i in 0..list.n_items() {
        let m: gtk4::gdk::Monitor = list.item(i).unwrap().downcast().unwrap();
        if want == "all" || m.connector().as_deref() == Some(want.as_str()) {
            out.push(m);
        }
    }
    // Unknown/typo connector → better all than nothing.
    if out.is_empty() {
        for i in 0..list.n_items() {
            out.push(list.item(i).unwrap().downcast().unwrap());
        }
    }
    out
}

/// Generic slide-in card: any caller (appointment reminders, the
/// notification server, future system messages) feeds it an optional
/// icon, a headline and a body line. One card per target monitor —
/// each animates in from that monitor's right edge, holds ~18 s and
/// slides back out.
pub fn slide_in(icon: Option<&str>, title: &str, body_text: &str) {
    play_sound();
    for m in target_monitors() {
        spawn_card(&m, icon, title, body_text);
    }
}

/// Build the card for one output and run its slide animation.
fn spawn_card(monitor: &gtk4::gdk::Monitor, icon: Option<&str>, title: &str, body_text: &str) {
    let win = gtk4::Window::new();
    win.init_layer_shell();
    win.set_layer(Layer::Overlay);
    win.set_monitor(Some(monitor));
    win.set_anchor(Edge::Right, true);
    win.set_anchor(Edge::Top, true);
    // KeyboardMode::None — a reminder must never steal input focus.
    win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    win.set_margin(Edge::Top, 12);
    win.set_default_width(CARD_WIDTH);
    win.set_can_focus(false);

    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    card.set_margin_top(10);
    card.set_margin_bottom(10);
    card.set_margin_start(12);
    card.set_margin_end(12);
    card.add_css_class("ragnar-raven-reminder");
    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    if let Some(name) = icon.filter(|n| !n.is_empty()) {
        let img = gtk4::Image::from_icon_name(name);
        img.set_pixel_size(24);
        top.append(&img);
    }
    let head = gtk4::Label::new(None);
    head.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
    head.set_halign(gtk4::Align::Start);
    top.append(&head);
    card.append(&top);
    let body = gtk4::Label::new(None);
    body.set_markup(&glib::markup_escape_text(body_text));
    body.set_halign(gtk4::Align::Start);
    body.set_wrap(true);
    card.append(&body);
    win.set_child(Some(&card));

    // Slide animation driven by the frame clock: right margin runs
    // -width → 8 (in), holds, then 8 → -width (out) before close.
    let state = Rc::new(std::cell::Cell::new(0u64)); // ms since map
    let st = state.clone();
    let w = win.clone();
    win.add_tick_callback(move |_, _clock| {
        let ms = st.get() + 16;
        st.set(ms);
        const SLIDE: u64 = 350;
        let hold = hold_ms();
        let total = SLIDE * 2 + hold;
        let off = if ms < SLIDE {
            // ease in
            -CARD_WIDTH as f64 + (CARD_WIDTH as f64 + 8.0) * (ms as f64 / SLIDE as f64)
        } else if ms < SLIDE + hold {
            8.0
        } else {
            // ease out
            let t = (ms - SLIDE - hold) as f64 / SLIDE as f64;
            8.0 - (CARD_WIDTH as f64 + 8.0) * t.min(1.0)
        };
        w.set_margin(Edge::Right, off.round() as i32);
        if ms >= total {
            w.close();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    win.present();
}
