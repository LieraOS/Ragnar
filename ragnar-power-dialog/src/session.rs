// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — session-manager handshake (--daemon)
//
// Port of the daemon's LibSession.register_with_session: registers
// as a session client at org.gnome.SessionManager, then answers the
// private client's QueryEndSession/EndSession signals with
// EndSessionResponse and quits on Stop — the contract the session
// manager needs to end the session cleanly.

use gtk4::gio;
use gtk4::gio::DBusConnection;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "ragnar-power-dialog";

/// Registers this process with the session manager and wires the
/// private-client signals: QueryEndSession/EndSession → confirm and
/// quit; Stop → quit immediately.
pub fn register_with_session(conn: &DBusConnection, app: &adw::Application) {
    let start_id = std::env::var("DESKTOP_AUTOSTART_ID").unwrap_or_default();
    if start_id.is_empty() {
        eprintln!("ragnar-power-dialog: DESKTOP_AUTOSTART_ID not set");
    }
    // Ask the session manager for our private client object path.
    let app2 = app.clone();
    let conn2 = conn.clone();
    conn.call(
        Some("org.gnome.SessionManager"),
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
        "RegisterClient",
        Some(&(APP_ID, &start_id).to_variant()),
        // Returns (o) — the client path wrapped in a tuple.
        glib::VariantTy::new("(o)").ok(),
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
        move |res| match res {
            Ok(v) => {
                let path = v.child_value(0).get::<String>().unwrap_or_default();
                if path.is_empty() {
                    eprintln!("ragnar-power-dialog: session gave no client path");
                    return;
                }
                wire_client(&conn2, &app2, path);
            }
            Err(e) => eprintln!("ragnar-power-dialog: RegisterClient failed: {e}"),
        },
    );
}

/// Proxies the private client object and connects its session-ending
/// signals — mirroring the legacy daemon's end_session handling.
fn wire_client(conn: &DBusConnection, app: &adw::Application, path: String) {
    let respond = |conn: &DBusConnection, path: &str| {
        conn.call(
            Some("org.gnome.SessionManager"),
            path,
            "org.gnome.SessionManager.ClientPrivate",
            "EndSessionResponse",
            Some(&(true, "").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
            |_| {},
        );
    };
    for signal in ["QueryEndSession", "EndSession"] {
        let (conn2, path2, respond2) = (conn.clone(), path.clone(), respond);
        let app2 = app.clone();
        conn.signal_subscribe(
            Some("org.gnome.SessionManager"),
            Some("org.gnome.SessionManager.ClientPrivate"),
            Some(signal),
            Some(&path),
            None,
            gio::DBusSignalFlags::NONE,
            move |_, _, _, _, _, _| {
                respond2(&conn2, &path2);
                app2.quit();
            },
        );
    }
    {
        let (conn2, path2) = (conn.clone(), path.clone());
        let app2 = app.clone();
        conn.signal_subscribe(
            Some("org.gnome.SessionManager"),
            Some("org.gnome.SessionManager.ClientPrivate"),
            Some("Stop"),
            Some(&path),
            None,
            gio::DBusSignalFlags::NONE,
            move |_, _, _, _, _, _| {
                let _ = (&conn2, &path2);
                app2.quit();
            },
        );
    }
}
