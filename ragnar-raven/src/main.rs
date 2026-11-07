//! ragnar-raven — the Ragnar sidebar (calendar, appointments,
//! notification history), rebuilt in Rust/GTK4 as a layer-shell
//! surface docked to the right screen edge.

mod dialog;
mod i18n;
mod notify;
mod reminder;
mod sidebar;
mod store;

use gtk4::glib;
use gtk4::prelude::*;

/// Read the `--day YYYYMMDD` option the taskbar passes when opening
/// Raven for a picked calendar day. Falls back to today for a bare
/// launch or a malformed value.
fn requested_day(cmd: &gtk4::gio::ApplicationCommandLine) -> glib::DateTime {
    let today = || glib::DateTime::now_local().unwrap();
    let v: Option<String> = cmd.options_dict().lookup("day").ok().flatten();
    let Some(v) = v else { return today() };
    if v.len() != 8 {
        return today();
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        v[0..4].parse(),
        v[4..6].parse(),
        v[6..8].parse(),
    ) else {
        return today();
    };
    glib::DateTime::from_local(y, m, d, 12, 0, 0.0)
        .ok()
        .unwrap_or_else(today)
}

thread_local! {
    /// Process-wide shared appointment store (see above).
    static STORE: std::cell::RefCell<Option<store::Store>> =
        const { std::cell::RefCell::new(None) };
}

fn main() {
    i18n::init();
    let app = gtk4::Application::builder()
        .application_id("org.lieraos.ragnar.Raven")
        // HANDLES_COMMAND_LINE gives us single-instance D-Bus
        // forwarding for free: `ragnar-raven --day …` on a running
        // instance is delivered to that instance over D-Bus instead
        // of spawning a second process.
        .flags(gtk4::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    // Declare --day so GApplication's option parser accepts it; the
    // value arrives in the command-line options dict, forwarded over
    // D-Bus to a running instance when there is one.
    app.add_main_option(
        "day",
        glib::Char::from(b'd'),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Open the appointment view for the given day (YYYYMMDD)",
        Some("YYYYMMDD"),
    );
    app.connect_command_line(|app, cmd| {
        // Dual role: without --day the process is the background
        // reminder daemon (hold keeps it alive without any window);
        // with --day it additionally opens the day view. A running
        // daemon instance receives later --day calls over D-Bus and
        // just opens another window — never a second process.
        // hold() returns a guard — dropping it releases the hold and
        // a windowless daemon would exit, so keep it deliberately.
        let guard = app.hold();
        std::mem::forget(guard);
        // One shared store per process: the daemon loop and every
        // opened day window must see the same in-memory state —
        // reloading per activation would hide fresh edits from the
        // reminder loop.
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            if s.is_none() {
                *s = Some(store::Store::load());
            }
            let store = s.clone().unwrap();
            reminder::start(&store);
            notify::start();
            if cmd.options_dict().contains("day") {
                let day = requested_day(cmd);
                sidebar::open(app, &store, &day);
            }
        });
        0
    });
    app.run();
}
