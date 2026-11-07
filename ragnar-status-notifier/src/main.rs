// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-status-notifier — StatusNotifierWatcher daemon
//
// Rust replacement for the StatusNotifier part of the legacy
// ragnar-daemon: owns the freedesktop and kde watcher names and
// brokers tray-item/host registration for the Ragnar desktop.

mod registry;
mod state;
mod svc;

fn main() {
    // Session-bus availability is the only hard requirement —
    // objects register once the names are owned.
    if gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).is_err() {
        eprintln!("ragnar-status-notifier: no session bus");
        std::process::exit(1);
    }
    let _owners = svc::own_names();
    glib::MainLoop::new(None, false).run();
}
