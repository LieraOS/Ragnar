// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-nightlight — night light manager daemon
//
// Rust replacement for the NightLightManager part of the legacy
// ragnar-daemon: watches org.gnome.settings-daemon.plugins.color,
// keeps gammastep's local config in sync and starts/stops the
// gammastep process accordingly. No D-Bus surface — pure settings
// watcher, same as the original.

mod config;
mod gammastep;

fn main() {
    // Without the color schema there is nothing to drive — exiting
    // cleanly is correct (autostart won't loop-spawn).
    let Some(settings) = config::settings() else {
        eprintln!("ragnar-nightlight: color schema missing, exiting");
        return;
    };
    config::ensure_local();

    // Apply current state once at startup, then on every change —
    // the legacy service did the same via its initial callback.
    apply(&settings);
    // Debounce: a settings panel writes several keys per change —
    // without collapsing, each one would respawn gammastep. The
    // one-shot timer clears its own id when firing, otherwise the
    // next change would remove() an already-spent source (panic).
    let pending = std::rc::Rc::new(std::cell::RefCell::new(None::<glib::SourceId>));
    let pending2 = pending.clone();
    config::watch(&settings, move |s| {
        if let Some(id) = pending2.borrow_mut().take() {
            id.remove();
        }
        let s = s.clone();
        let pending3 = pending2.clone();
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            pending3.borrow_mut().take();
            apply(&s);
        });
        *pending2.borrow_mut() = Some(id);
    });

    glib::MainLoop::new(None, false).run();
}

/// Evaluates the enabled flag and starts/stops gammastep.
fn apply(settings: &gio::Settings) {
    if config::enabled(settings) {
        // gammastep never re-reads its config at runtime — a changed
        // ini only takes effect on a fresh process, so every settings
        // change means stop → rewrite → respawn.
        gammastep::stop();
        config::update(settings);
        gammastep::run();
    } else {
        gammastep::stop();
    }
}
