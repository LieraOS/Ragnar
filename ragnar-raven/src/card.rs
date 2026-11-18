//! The slide-in notification card — presentation surface shared by
//! appointment reminders, bus notifications and watcher events:
//! category accents, per-monitor overlay windows, the card registry
//! (CloseNotification/replaces_id) and the card sounds.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Width of the reminder card — small, readable, upper-right area.
const CARD_WIDTH: i32 = 300;

/// Visual category of a notification — picks the accent colour
/// (`color_<key>` under `[notifications]`) painted as the card's
/// left border. Internal producers (reminders, watchers) pass their
/// category explicitly; bus notifications are always Default.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Default,
    Mount,
    Update,
    Appointment,
}

impl Category {
    /// ini key under `[notifications]` holding this category's
    /// colour; absent or unparseable keeps the stock card look.
    fn ini_key(self) -> &'static str {
        match self {
            Self::Default => "color_default",
            Self::Mount => "color_mount",
            Self::Update => "color_update",
            Self::Appointment => "color_appointment",
        }
    }
}

thread_local! {
    /// CSS text already registered on the display — one provider
    /// per distinct accent so repeated cards don't pile providers.
    static ACCENT_CSS: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());
}

/// Paint the category's accent colour as a 4 px bar on the card's
/// left edge. Each category gets its own class (`raven-accent-*`)
/// so colours change per category without touching sibling cards.
fn apply_accent(card: &gtk4::Box, cat: Category) {
    let Some(hex) = crate::util::ini_get("notifications", cat.ini_key()) else {
        return;
    };
    if gtk4::gdk::RGBA::parse(&hex).is_err() {
        return;
    }
    let class = format!("raven-accent-{}", cat.ini_key());
    ACCENT_CSS.with(|reg| {
        let css = format!(".{class} {{ border-left: 4px solid {hex}; }}");
        if reg.borrow_mut().insert(css.clone()) {
            let prov = gtk4::CssProvider::new();
            prov.load_from_data(&css);
            if let Some(disp) = gtk4::gdk::Display::default() {
                gtk4::style_context_add_provider_for_display(
                    &disp,
                    &prov,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        }
    });
    card.add_css_class(&class);
}

thread_local! {
/// On-screen cards keyed by their notification id — one entry per
    /// notification, one window per target monitor. Lets the
    /// org.freedesktop.Notifications handlers implement
    /// CloseNotification and replaces_id for real.
    static CARDS: std::cell::RefCell<HashMap<u32, Vec<gtk4::Window>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Session-bus connection kept for emitting NotificationClosed —
    /// set once when notify.rs registers the object.
    static CONN: std::cell::RefCell<Option<gtk4::gio::DBusConnection>> =
        const { std::cell::RefCell::new(None) };
}

/// Remember the bus connection so cards can emit NotificationClosed
/// when they expire or get replaced.
pub fn set_conn(conn: &gtk4::gio::DBusConnection) {
    CONN.with(|c| *c.borrow_mut() = Some(conn.clone()));
}

/// Emit NotificationClosed(id, reason): 1 = expired, 3 = closed on
/// request (CloseNotification or replaced). Spec-mandated signal —
/// clients like chat apps rely on it to clear their own bookkeeping.
fn emit_closed(id: u32, reason: u32) {
    CONN.with(|c| {
        if let Some(conn) = c.borrow().as_ref() {
            let _ = conn.emit_signal(
                None::<&str>,
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
                "NotificationClosed",
                Some(&(id, reason).to_variant()),
            );
        }
    });
}

/// Close every on-screen card belonging to notification `id` and
/// emit NotificationClosed(id, 3) — every path that reaches this
/// function is a close-on-request (CloseNotification call or a
/// replaces_id overwrite). Returns true when a card was open.
pub fn close_cards(id: u32) -> bool {
    let closed = CARDS.with(|c| match c.borrow_mut().remove(&id) {
        Some(wins) => {
            for w in wins {
                w.close();
            }
            true
        }
        None => false,
    });
    if closed {
        emit_closed(id, 3);
    }
    closed
}

/// How long the card stays on screen — `[notifications] duration_s`
/// in settings.ini, default 18 s.
fn hold_ms() -> u64 {
    let s = crate::util::ini_int("notifications", "duration_s").unwrap_or(18);
    (s.max(2) as u64) * 1000
}

/// Optional notification sound — `[notifications] sound` holds a
/// freedesktop sound-event id (e.g. `dialog-information`, `bell`);
/// empty/absent means silent (the default). Played once per slide,
/// not per monitor, through libcanberra in-process (see sound.rs).
fn play_sound() {
    let name = crate::util::ini_get("notifications", "sound").unwrap_or_default();
    if name.is_empty() || name == "none" {
        return;
    }
    crate::sound::play_event(name.as_str());
}

/// Which sound plays for a slide: a per-appointment file wins over
/// the global event sound (and over silence).
fn play_card_sound(sound: Option<&std::path::Path>) {
    match sound {
        Some(p) => crate::sound::play_file(p),
        None => play_sound(),
    }
}

/// Which monitors get the card. `monitor = all` (or unset) puts one
/// card on every output; a connector name (e.g. `DP-3`) pins it to
/// that monitor. ragnar-settings will write this key later — the two
/// components stay decoupled through the shared settings.ini.
fn target_monitors() -> Vec<gtk4::gdk::Monitor> {
    let want = crate::util::ini_get("notifications", "monitor").unwrap_or_else(|| "all".into());
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
/// notification server, the event watchers) feeds it a category,
/// an optional icon, a headline and a body line. `sound` overrides
/// the global notification sound for this card (per-appointment
/// custom file); None plays the configured event sound. One card
/// per target monitor — each animates in from that monitor's right
/// edge, holds ~18 s and slides back out.
pub fn slide_in(
    cat: Category,
    icon: Option<&gio::Icon>,
    title: &str,
    body_text: &str,
    sound: Option<&std::path::Path>,
) {
    play_card_sound(sound);
    for m in target_monitors() {
        spawn_card(&m, cat, icon, title, body_text, None);
    }
}

/// Slide-in for a real notification: tracked under `id` so
/// CloseNotification and replaces_id can find and close it later.
/// A previous card with the same id is closed first.
pub fn slide_in_notification(
    id: u32,
    cat: Category,
    icon: Option<&gio::Icon>,
    title: &str,
    body_text: &str,
) {
    close_cards(id);
    play_sound();
    for m in target_monitors() {
        spawn_card(&m, cat, icon, title, body_text, Some(id));
    }
}

/// Build the card for one output and run its slide animation.
/// `id` registers the window in the notification-card map.
fn spawn_card(
    monitor: &gtk4::gdk::Monitor,
    cat: Category,
    icon: Option<&gio::Icon>,
    title: &str,
    body_text: &str,
    id: Option<u32>,
) {
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
    apply_accent(&card, cat);
    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    if let Some(gicon) = icon {
        let img = gtk4::Image::from_gicon(gicon);
        img.set_pixel_size(24);
        top.append(&img);
    }
    let head = gtk4::Label::new(None);
    head.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
    head.set_halign(gtk4::Align::Start);
    top.append(&head);
    card.append(&top);
    let body = gtk4::Label::new(None);
    // The freedesktop spec allows body markup — we advertise
    // body-markup in GetCapabilities. Render it when the client sent
    // valid markup, fall back to plain text otherwise (a bare '&'
    // would otherwise surface as a broken string).
    match gtk4::pango::parse_markup(body_text, '_') {
        Ok(_) => body.set_markup(body_text),
        Err(_) => body.set_text(body_text),
    }
    body.set_halign(gtk4::Align::Start);
    body.set_wrap(true);
    card.append(&body);
    win.set_child(Some(&card));

    // Track notification cards under their id; expiry removes them.
    if let Some(nid) = id {
        CARDS.with(|c| {
            c.borrow_mut().entry(nid).or_default().push(win.clone());
        });
    }

    // Slide animation driven by the frame clock: right margin runs
    // -width → 8 (in), holds, then 8 → -width (out) before close.
    let state = Rc::new(std::cell::Cell::new(0u64)); // ms since map
    let st = state.clone();
    let w = win.clone();
    let hold = hold_ms(); // read once — not per frame
    win.add_tick_callback(move |_, _clock| {
        let ms = st.get() + 16;
        st.set(ms);
        const SLIDE: u64 = 350;
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
            // Natural expiry: drop the window from the registry and
            // emit NotificationClosed(id, 1) — but only if the card
            // was still tracked; a CloseNotification-released card
            // already signalled reason 3.
            if let Some(nid) = id {
                let expired = CARDS.with(|c| {
                    let mut map = c.borrow_mut();
                    let Some(v) = map.get_mut(&nid) else {
                        return false;
                    };
                    let tracked = v.iter().any(|h| h == &w);
                    v.retain(|h| h != &w);
                    if v.is_empty() {
                        map.remove(&nid);
                    }
                    tracked
                });
                if expired {
                    emit_closed(nid, 1);
                }
            }
            w.close();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    win.present();
}
