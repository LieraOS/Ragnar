// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-mpris — which MPRIS player media keys control right now
//
// Tracks org.mpris.MediaPlayer2.* name owners on the session bus.
// The "current" player is the most recently appeared one that is
// still owned — same selection rule the legacy MprisTracker used
// (last seen wins). Players that vanish are dropped instantly.

use gio::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;

const PREFIX: &str = "org.mpris.MediaPlayer2.";

thread_local! {
    /// Known player bus names, most recently seen first.
    static PLAYERS: RefCell<VecDeque<String>> = RefCell::new(VecDeque::new());
}

/// Subscribes to NameOwnerChanged for mpris names and seeds the
/// list once from ListNames.
pub fn init(conn: &gio::DBusConnection) {
    // Seed: players that are already on the bus when we start.
    if let Ok(v) = conn.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ListNames",
        None,
        Some(glib::VariantTy::new("(as)").unwrap()),
        gio::DBusCallFlags::NONE,
        5_000,
        gio::Cancellable::NONE,
    ) {
        if let Some(names) = v.child_value(0).get::<Vec<String>>() {
            PLAYERS.with(|p| {
                let mut p = p.borrow_mut();
                for n in names.into_iter().filter(|n| n.starts_with(PREFIX)) {
                    p.push_front(n);
                }
            });
        }
    }
    // Track additions/removals from here on.
    conn.signal_subscribe(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        None::<&str>,
        gio::DBusSignalFlags::NONE,
        |_, _, _, _, _, params| {
            let Some((name, _old, new)) = params.get::<(String, String, String)>() else {
                return;
            };
            
            if !name.starts_with(PREFIX) {
                return;
            }
            PLAYERS.with(|p| {
                let mut p = p.borrow_mut();
                p.retain(|n| n != &name);
                if !new.is_empty() {
                    p.push_front(name);
                }
            });
        },
    );
}

/// The bus name of the player to control right now, if any.
pub fn current() -> Option<String> {
    PLAYERS.with(|p| p.borrow().front().cloned())
}
