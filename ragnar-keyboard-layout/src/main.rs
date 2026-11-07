// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-keyboard-layout — keyboard-layout broker daemon
//
// Rust replacement for the KeyboardLayoutManager part of the legacy
// ragnar-daemon: owns org.lieraos.ragnar.KeyboardLayout and brokers
// layout requests between the panel applet, compositor keybinds and
// the environment bridge which applies them.

mod state;
mod svc;

fn main() {
    // Session-bus availability is the only hard requirement —
    // the object registers once the name is owned.
    if gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).is_err() {
        eprintln!("ragnar-keyboard-layout: no session bus");
        std::process::exit(1);
    }
    let _owner = svc::own_name();
    glib::MainLoop::new(None, false).run();
}
