// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-wallpaper — session wallpaper service
//
// Rust replacement for the background part of the legacy ragnar-daemon:
// reads org.gnome.desktop.background (picture-uri + picture-options),
// keeps a swaybg process running, respawns it on every settings
// change and mirrors the choice into AccountsService so the greeter
// shows the same wallpaper.

mod accounts;
mod background;
mod map;
mod outputs;
mod swaybg;

fn main() {
    // Apply once at session start, then stay resident and re-apply
    // on every GSettings change and on every per-monitor map write
    // by ragnar-settings.
    swaybg::apply(&background::current());
    background::watch(|b| swaybg::apply(&b));
    map::watch(|| swaybg::apply(&background::current()));
    // Watchdog: re-apply whenever every swaybg is gone (covers the
    // session-start race and any unexpected swaybg death).
    swaybg::watch_last();
    glib::MainLoop::new(None, false).run();
}
