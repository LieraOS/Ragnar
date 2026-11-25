// Copyright (C) Jörg Schröder
//! Reminder daemon + slide-in notification.
//!
//! A glib timer wakes every 30 s and compares every appointment's
//! VALARM offsets against the clock: when `start - offset` falls into
//! the current tick window and has not fired yet, a small overlay
//! window slides in from the right edge, stays ~18 s and slides back
//! out. Fired reminders are tracked per (uid, offset) so each VALARM
//! fires exactly once per process lifetime.

use gtk4::glib;
use std::collections::HashSet;

use crate::card::{self, Category};
use crate::i18n::tr;
use crate::store::Store;

thread_local! {
    /// (uid, offset-minutes, start-unix) triples that already fired
    /// this session. The start is part of the key so editing an
    /// appointment's time re-arms its reminders — without it a
    /// moved appointment would never remind again.
    static FIRED: std::cell::RefCell<HashSet<(String, i64, i64)>> =
        std::cell::RefCell::new(HashSet::new());
}

/// Whether the user globally enabled appointment notifications
/// (the same settings.ini the taskbar calendar menu toggles).
fn enabled() -> bool {
    crate::util::ini_bool("notifications", "enabled").unwrap_or(true)
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
            let Some(fire_at) = a.start.add(glib::TimeSpan::from_minutes(-*m)).ok() else {
                continue;
            };
            // Due if the fire time lies within the last 60 s (the
            // 30 s tick may run a little late). Mark as fired only
            // inside the due branch — marking future reminders would
            // skip them forever when their time actually comes.
            let diff = now.difference(&fire_at).as_seconds();
            if (0..60).contains(&diff) {
                let key = (a.uid.clone(), *m, a.start.to_unix());
                if FIRED.with(|f| f.borrow_mut().insert(key)) {
                    let when = a.start.format("%H:%M").unwrap();
                    // Title line first, optional note underneath.
                    let body = if a.desc.is_empty() {
                        format!("{}  {}", when, a.title)
                    } else {
                        format!("{}  {}\n{}", when, a.title, a.desc)
                    };
                    card::slide_in(
                        Category::Appointment,
                        None,
                        &tr("Termin"),
                        &body,
                        if a.sound.is_empty() {
                            None
                        } else {
                            Some(std::path::Path::new(&a.sound))
                        },
                    );
                }
            }
        }
    }
}
