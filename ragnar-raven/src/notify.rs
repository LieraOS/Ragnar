//! freedesktop notification server (org.freedesktop.Notifications).
//!
//! Raven owns the well-known name on the session bus — taken over
//! from ragnar-daemon via REPLACE — and turns every incoming Notify
//! call into a history entry plus the right-edge slide-in card. This
//! replaces the legacy daemon's notification server function.

use gtk4::gio::{self, prelude::*};
use gtk4::glib;

use crate::reminder;
use std::collections::HashMap;

/// One stored notification as shown in the history section.
/// Serialized one-per-line into history.jsonl.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub when: String,
}

/// History file: one JSON object per line, appended chronologically.
/// JSONL keeps the file machine-readable for other tools — same idea
/// as the .ics appointment store being standard format.
fn history_file() -> std::path::PathBuf {
    let dir = glib::user_data_dir().join("ragnar-raven");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("history.jsonl")
}

/// Max entries kept in the file; older ones are trimmed on write.
const HISTORY_MAX: usize = 200;

/// Load persisted history (newest first) into memory. Called once at
/// daemon start so the day window isn't empty after a relogin.
fn load_history() {
    let Ok(text) = std::fs::read_to_string(history_file()) else {
        return;
    };
    let mut items: Vec<Item> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    items.reverse(); // file is append-ordered; memory is newest-first
    HISTORY.with(|h| *h.borrow_mut() = items);
}

/// Append one entry and trim the file to HISTORY_MAX if it grows
/// beyond it.
fn append_history(item: &Item) {
    let path = history_file();
    let mut keep: Vec<String> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        keep = text.lines().map(|l| l.to_string()).collect();
        if keep.len() >= HISTORY_MAX {
            keep.drain(..keep.len() - HISTORY_MAX + 1);
        }
    }
    if let Ok(line) = serde_json::to_string(item) {
        keep.push(line);
        let _ = std::fs::write(&path, keep.join("\n") + "\n");
    }
}

thread_local! {
    /// In-memory history (newest first), shown in the day window.
    static HISTORY: std::cell::RefCell<Vec<Item>> =
        std::cell::RefCell::new(Vec::new());
    /// Last issued notification id.
    static NEXT_ID: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

/// Newest-first snapshot of the notification history.
pub fn history() -> Vec<Item> {
    HISTORY.with(|h| h.borrow().clone())
}

/// freedesktop Notifications interface description.
const XML: &str = r#"
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities">
      <arg type="as" direction="out"/>
    </method>
    <method name="Notify">
      <arg type="s" direction="in"/>
      <arg type="u" direction="in"/>
      <arg type="s" direction="in"/>
      <arg type="s" direction="in"/>
      <arg type="s" direction="in"/>
      <arg type="as" direction="in"/>
      <arg type="a{sv}" direction="in"/>
      <arg type="i" direction="in"/>
      <arg type="u" direction="out"/>
    </method>
    <method name="CloseNotification">
      <arg type="u" direction="in"/>
    </method>
    <method name="GetServerInformation">
      <arg type="s" direction="out"/>
      <arg type="s" direction="out"/>
      <arg type="s" direction="out"/>
      <arg type="s" direction="out"/>
    </method>
    <signal name="NotificationClosed">
      <arg type="u"/><arg type="u"/>
    </signal>
    <signal name="ActionInvoked">
      <arg type="u"/><arg type="s"/>
    </signal>
  </interface>
</node>"#;

/// Register the object and take over the bus name.
pub fn start() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if ONCE.is_completed() {
        return;
    }
    ONCE.call_once(|| ());
    load_history();
    gio::bus_own_name(
        gio::BusType::Session,
        "org.freedesktop.Notifications",
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| register(&conn),
        |_, _| {},
        |_, _| {},
    );
}

/// Export /org/freedesktop/Notifications on the session bus.
fn register(conn: &gio::DBusConnection) {
    let info = gio::DBusNodeInfo::for_xml(XML).unwrap();
    let iface = info
        .lookup_interface("org.freedesktop.Notifications")
        .unwrap();
    let _ = conn
        .register_object("/org/freedesktop/Notifications", &iface)
        .method_call(|_c, _s, _p, _i, method, params, inv| dispatch(method, params, inv))
        .build();
}

/// Route one method call to its handler.
fn dispatch(
    method: &str,
    params: glib::Variant,
    inv: gio::DBusMethodInvocation,
) {
    match method {
        "GetCapabilities" => {
            let caps: Vec<String> =
                ["body", "body-markup", "icon-static", "persistence"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
            inv.return_value(Some(&(caps,).to_variant()));
        }
        "GetServerInformation" => {
            inv.return_value(Some(
                &(
                    "ragnar-raven",
                    "LieraOS",
                    env!("CARGO_PKG_VERSION"),
                    "1.2",
                )
                    .to_variant(),
            ));
        }
        "Notify" => {
            let p = params.get::<(
                String,
                u32,
                String,
                String,
                String,
                Vec<String>,
                HashMap<String, glib::Variant>,
                i32,
            )>();
            let Some((app, replaces, icon, summary, body, _a, _h, _t)) = p
            else {
                inv.return_dbus_error(
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "bad Notify signature",
                );
                return;
            };
            let id = if replaces != 0 {
                replaces
            } else {
                NEXT_ID.with(|n| {
                    let v = n.get();
                    n.set(v + 1);
                    v
                })
            };
            let item = Item {
                app: app.clone(),
                icon: icon.clone(),
                summary: summary.clone(),
                body: body.clone(),
                when: glib::DateTime::now_local()
                    .and_then(|d| d.format("%d.%m.%Y %H:%M"))
                    .map(|g| g.to_string())
                    .unwrap_or_default(),
            };
            append_history(&item);
            HISTORY.with(|h| {
                h.borrow_mut().insert(0, item);
                h.borrow_mut().truncate(HISTORY_MAX);
            });
            reminder::slide_in(
                if icon.is_empty() { None } else { Some(icon.as_str()) },
                &summary,
                &body,
            );
            inv.return_value(Some(&(id,).to_variant()));
        }
        "CloseNotification" => inv.return_value(None),
        _ => inv.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            method,
        ),
    }
}
