// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-keyboard-layout — the single piece of in-memory state
//
// Mirrors the legacy service: CurrentLayout lives in memory only. The
// compositor bridge owns the on-disk truth (XKB_DEFAULT_LAYOUT in the
// session environment file) and pushes the active layout back through
// the writable CurrentLayout property.

use std::cell::RefCell;

thread_local! {
    /// Layout currently reported to consumers (e.g. the panel applet).
    static CURRENT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Stores the layout and reports whether it actually changed — the
/// bridge publishes on every environment rewrite, so only a real
/// change must produce a PropertiesChanged signal.
pub fn set(layout: &str) -> bool {
    CURRENT.with(|c| {
        let mut c = c.borrow_mut();
        if *c == layout {
            return false;
        }
        *c = layout.to_string();
        true
    })
}

/// The layout value handed out on property reads.
pub fn get() -> String {
    CURRENT.with(|c| c.borrow().clone())
}
