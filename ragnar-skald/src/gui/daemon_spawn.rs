//! Shared fx-daemon spawn helper used by the effect widgets.
//!
//! Both effect sections (output chain, mic chain) spawn the
//! `ragnar-skald-fx` daemon the same way: once per
//! [`RESPAWN_COOLDOWN`] window, errors land on a status label
//! instead of panicking or retrying in a loop.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::Label;
use tracing::warn;

use crate::fxdaemon;

/// Minimum delay between daemon spawn attempts — guards against a
/// crash-loop when the daemon binary is missing or broken.
pub const RESPAWN_COOLDOWN: Duration = Duration::from_secs(5);

/// Spawn the fx daemon unless a recent attempt already ran. `status`
/// receives the error message on failure.
pub fn spawn_daemon(last_spawn: &Rc<Cell<Option<Instant>>>, status: &Label) {
    let due = last_spawn
        .get()
        .map(|t| t.elapsed() >= RESPAWN_COOLDOWN)
        .unwrap_or(true);
    if !due {
        return;
    }
    last_spawn.set(Some(Instant::now()));
    if let Err(e) = fxdaemon::ensure_running() {
        warn!("Failed to spawn fx daemon: {e:#}");
        status.set_label("Effekt-Dienst konnte nicht gestartet werden");
    }
}
