// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — resident daemon mode (--daemon)
//
// Replaces the legacy ragnar-power-dialog daemon: stays resident in
// the session, owns the D-Bus names the session talks to and shows
// the same Rust/GTK4 dialog on demand.
//
//   org.gnome.Shell          → /org/gnome/SessionManager/EndSessionDialog
//     the contract ragnar-session speaks: Open() shows the dialog,
//     a session-ending choice answers with Confirmed* so the session
//     performs the deed — we do not execute it ourselves.
//   org.lieraos.ragnar.Session.EndSessionDialog → same interface on
//     the ragnar name (the legacy shim only proxied; we serve both).
//   org.lieraos.ragnar.PowerDialog → Toggle() for callers that poke
//     the dialog directly (compat).

use crate::dbus_shim;
use crate::session;
use gtk4::gio;
use gtk4::gio::DBusConnection;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;

/// True once --daemon mode has taken over the bus names — only then
/// may clicks be answered via the EndSessionDialog contract.
static DAEMON_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    /// The dialog window while it is shown (for Close/Toggle) plus a
    /// flag whether it was raised by the session's Open call — a
    /// session-opened dialog reports Closed when it goes away.
    static STATE: RefCell<(Option<adw::ApplicationWindow>, bool, bool)> = RefCell::new((None, false, false));
}

/// Actions answered via the EndSessionDialog contract instead of
/// being executed locally — the session performs them.
const SESSION_ACTIONS: &[(&str, &str)] = &[
    ("logout", "ConfirmedLogout"),
    ("reboot", "ConfirmedReboot"),
    ("shutdown", "ConfirmedShutdown"),
];

/// Runs the dialog as the resident session daemon. Owns the bus
/// names, registers the objects and keeps the GTK app alive;
/// activate/Open/Toggle all funnel into window::present.
pub fn run(app: &adw::Application) -> glib::ExitCode {
    DAEMON_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
    // Without HANDLES_COMMAND_LINE, GApplication fires activate on
    // startup — that would pop the dialog up at every login. Handle
    // the command line explicitly so the daemon starts hidden; the
    // window still opens via D-Bus activate() calls.
    app.set_flags(app.flags() | gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    app.connect_command_line(|_, _| 0);
    // HANDLES_COMMAND_LINE makes GApplication quit right after the
    // startup cycle unless something holds it — the daemon must stay
    // resident for the session's lifetime.
    let _hold = app.hold();
    let Some(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok() else {
        eprintln!("ragnar-power-dialog --daemon: no session bus");
        return glib::ExitCode::FAILURE;
    };
    let node = gio::DBusNodeInfo::for_xml(dbus_shim::NODE_XML).expect("valid node xml");
    register(
        &conn,
        &node,
        "org.lieraos.ragnar.PowerDialog",
        dbus_shim::PATH_POWER,
        app,
    );
    register(
        &conn,
        &node,
        "org.gnome.SessionManager.EndSessionDialog",
        dbus_shim::PATH_GNOME,
        app,
    );
    register(
        &conn,
        &node,
        "org.lieraos.ragnar.Session.EndSessionDialog",
        dbus_shim::PATH_RAGNAR,
        app,
    );
    for name in [
        dbus_shim::NAME_SHELL,
        dbus_shim::NAME_ENDSESSION,
        dbus_shim::NAME_POWER,
    ] {
        gio::bus_own_name(
            gio::BusType::Session,
            name,
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, _| {},
            |_, name| eprintln!("ragnar-power-dialog --daemon: lost bus name {name}"),
        );
    }
    // Take over the daemon's libsession handshake: register as a
    // session client so QueryEndSession/EndSession/Stop reach us —
    // without this the session manager cannot end the session.
    session::register_with_session(&conn, app);
    // GTK must not see --daemon; hand it only argv[0].
    app.run_with_args(&["ragnar-power-dialog"])
}

/// Registers `iface` at `path` and wires its methods into the GTK
/// app: Open/present, Close/hide, Toggle/flip.
fn register(
    conn: &DBusConnection,
    node: &gio::DBusNodeInfo,
    iface: &str,
    path: &str,
    app: &adw::Application,
) {
    let Some(info) = node.lookup_interface(iface) else {
        return;
    };
    let a = app.clone();
    conn.register_object(path, &info)
        .method_call(move |_conn, _s, _p, _i, method, _params, inv| {
            match method {
                "Open" => {
                    STATE.with(|s| s.borrow_mut().1 = true);
                    a.activate();
                }
                "Close" => {
                    close_current();
                }
                "Toggle" => {
                    if !close_current() {
                        a.activate();
                    }
                }
                _ => {}
            }
            inv.return_value(None);
        })
        .build();
}

/// Closes the tracked window if one is up; returns whether it closed.
fn close_current() -> bool {
    STATE.with(|s| {
        if let Some(w) = s.borrow_mut().0.take() {
            w.close();
            return true;
        }
        false
    })
}

/// Handles an action click in daemon mode: session-ending actions
/// answer the contract with Confirmed* on both object paths (the
/// session executes the deed); everything else runs locally.
/// Returns true when the click was consumed here.
pub fn handle_action(action: &'static str) -> bool {
    if !DAEMON_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    // Only a dialog raised by the session's Open() answers via the
    // contract — Toggle/activate must execute the action locally,
    // exactly like the legacy daemon did.
    if !STATE.with(|s| s.borrow().1) {
        return false;
    }
    STATE.with(|s| s.borrow_mut().2 = true);
    let Some((_, signal)) = SESSION_ACTIONS.iter().find(|(id, _)| *id == action) else {
        return false;
    };
    if let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
        for path in [dbus_shim::PATH_GNOME, dbus_shim::PATH_RAGNAR] {
            for iface in [
                "org.gnome.SessionManager.EndSessionDialog",
                "org.lieraos.ragnar.Session.EndSessionDialog",
            ] {
                let _ = conn.emit_signal(None, path, iface, signal, None::<&glib::Variant>);
            }
        }
    }
    // Deliberately false: the signal only informs the session that
    // the user confirmed — it does not perform the deed. The caller
    // still runs the action via the normal local path
    // (ragnar-session-quit/logind), same as the legacy dialog did;
    // emitting alone left a torn-down session at a black screen.
    false
}

/// Called by the window when it is shown/closed so Toggle/Close keep
/// working; emits Closed when a session-raised dialog goes away.
pub fn track(win: &adw::ApplicationWindow) {
    STATE.with(|s| s.borrow_mut().0 = Some(win.clone().upcast::<adw::ApplicationWindow>()));
    win.connect_close_request(|_w| {
        let session_raised = STATE.with(|s| {
            let mut st = s.borrow_mut();
            let was = st.1;
            st.0 = None;
            st.1 = false;
            was
        });
        if session_raised {
            if let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
                for (path, iface) in [
                    (
                        dbus_shim::PATH_GNOME,
                        "org.gnome.SessionManager.EndSessionDialog",
                    ),
                    (
                        dbus_shim::PATH_RAGNAR,
                        "org.lieraos.ragnar.Session.EndSessionDialog",
                    ),
                ] {
                    let _ = conn.emit_signal(None, path, iface, "Closed", None::<&glib::Variant>);
                }
            }
        }
        glib::Propagation::Proceed
    });
}
