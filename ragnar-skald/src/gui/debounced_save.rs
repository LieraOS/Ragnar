// Copyright (C) Jörg Schröder
//! Debounced settings persistence shared by the effects widgets.
//!
//! Slider drags generate a stream of changes; writes to
//! `settings.json` are coalesced so one save runs at most
//! [`SAVE_DEBOUNCE`] after the last change. The `pending` slot is owned
//! by the calling widget and prevents piling up timer sources.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use tracing::warn;

use crate::config::{AppSettings, ConfigPersistence};

/// Debounce window for settings writes.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

/// New pending-slot for one widget — pass to [`debounced_save`].
pub fn pending_slot() -> Rc<RefCell<Option<glib::SourceId>>> {
    Rc::new(RefCell::new(None))
}

/// Closure that schedules a debounced settings write.
///
/// Cheap to clone — every handler needing "persist soon" gets its own
/// copy; the shared `pending` slot coalesces them.
pub fn debounced_save(
    settings: Rc<RefCell<AppSettings>>,
    persist: Rc<ConfigPersistence>,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
) -> impl Fn() + Clone {
    move || {
        let mut slot = pending.borrow_mut();
        if slot.is_some() {
            return; // a save is already queued
        }
        let settings = Rc::clone(&settings);
        let persist = Rc::clone(&persist);
        let pending2 = Rc::clone(&pending);
        *slot = Some(glib::timeout_add_local_once(SAVE_DEBOUNCE, move || {
            *pending2.borrow_mut() = None;
            if let Err(e) = persist.save(&settings.borrow()) {
                warn!("Failed to save settings: {e:#}");
            }
        }));
    }
}

/// Immediate settings write (rollback paths and explicit toggles).
pub fn save_now(settings: &Rc<RefCell<AppSettings>>, persist: &Rc<ConfigPersistence>) {
    if let Err(e) = persist.save(&settings.borrow()) {
        warn!("Failed to save settings: {e:#}");
    }
}
