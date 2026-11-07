//! Inhibitor tracking for the SessionManager `Inhibit` API.
//!
//! Callers receive a monotonically increasing cookie that identifies
//! their inhibition request; `IsInhibited` reports whether a given
//! action category is currently blocked.

use std::collections::HashMap;

/// One registered inhibition request.
#[derive(Debug)]
pub struct Inhibitor {
    /// Cookie handed back to the caller (`Inhibit` return value).
    pub cookie: u32,
    /// Well-known name of the requesting client.
    pub bus_name: String,
    /// Human-readable reason shown in confirmation dialogs.
    pub reason: String,
    /// Bitmask of blocked actions (logout=1, suspend=2, shutdown=4, ...).
    pub flags: u32,
}

/// Registry of all active inhibitors, keyed by cookie.
#[derive(Default)]
pub struct InhibitorRegistry {
    next_cookie: u32,
    entries: HashMap<u32, Inhibitor>,
}

impl InhibitorRegistry {
    /// Registers a new inhibitor and returns its cookie.
    pub fn add(&mut self, bus_name: String, reason: String, flags: u32) -> u32 {
        self.next_cookie += 1;
        let cookie = self.next_cookie;
        self.entries.insert(
            cookie,
            Inhibitor {
                cookie,
                bus_name,
                reason,
                flags,
            },
        );
        cookie
    }

    /// Removes the inhibitor identified by `cookie`.
    pub fn remove(&mut self, cookie: u32) {
        self.entries.remove(&cookie);
    }

    /// Returns true if any active inhibitor blocks `flags`.
    pub fn is_inhibited(&self, flags: u32) -> bool {
        self.entries.values().any(|i| i.flags & flags != 0)
    }

    /// Bitmask of all currently inhibited actions.
    pub fn inhibited_actions(&self) -> u32 {
        self.entries.values().fold(0, |acc, i| acc | i.flags)
    }
}
