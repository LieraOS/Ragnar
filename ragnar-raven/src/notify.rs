//! freedesktop notification server (org.freedesktop.Notifications).
//!
//! Raven owns the well-known name on the session bus — taken over
//! from ragnar-daemon via REPLACE — and turns every incoming Notify
//! call into a history entry plus the right-edge slide-in card. This
//! replaces the legacy daemon's notification server function.

use gtk4::gio::{self, prelude::*};
use gtk4::glib;

use crate::card;
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
    /// Unix seconds — the real timestamp retention pruning works on;
    /// `when` is only the display string. `default` so entries
    /// written before this field existed still load (as age 0).
    #[serde(default)]
    pub ts: i64,
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

/// Retention cutoff as unix seconds — `[notifications] history_days`
/// in settings.ini: 0 keeps nothing, negative/absent keeps the full
/// history (previous behaviour). Entries older than the cutoff are
/// pruned from the file and hidden from every reader.
fn history_cutoff() -> Option<i64> {
    let days = crate::util::ini_int("notifications", "history_days").unwrap_or(-1);
    if days < 0 {
        return None;
    }
    let now = glib::DateTime::now_local()
        .map(|d| d.to_unix())
        .unwrap_or(0);
    Some(now - days * 86_400)
}

/// Drop entries older than the retention cutoff. Items still in
/// file order (oldest first).
fn prune_aged(items: &mut Vec<Item>) {
    if let Some(cut) = history_cutoff() {
        items.retain(|i| i.ts >= cut);
    }
}

/// Load persisted history (newest first) into memory. Called once at
/// daemon start so the day window isn't empty after a relogin. Aged
/// entries are pruned and the file rewritten when they were.
fn load_history() {
    let Ok(text) = std::fs::read_to_string(history_file()) else {
        return;
    };
    let mut items: Vec<Item> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let before = items.len();
    prune_aged(&mut items);
    if items.len() != before {
        // Rewrite the pruned file (still chronological) so readers
        // of history.jsonl — e.g. the taskbar bell — agree.
        let out: String = items
            .iter()
            .filter_map(|i| serde_json::to_string(i).ok())
            .map(|l| l + "\n")
            .collect();
        crate::util::write_atomic(&history_file(), &out);
    }
    items.reverse(); // file is append-ordered; memory is newest-first
    HISTORY.with(|h| *h.borrow_mut() = items);
}

/// Append one entry, prune aged ones, and trim the file to
/// HISTORY_MAX if it grows beyond it.
fn append_history(item: &Item) {
    let path = history_file();
    let mut lines: Vec<String> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&path) {
        lines = text.lines().map(|l| l.to_string()).collect();
    }
    // Prune by age before the count trim — retention is measured in
    // time, not line count.
    if history_cutoff().is_some() {
        let mut items: Vec<Item> = lines
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        prune_aged(&mut items);
        lines = items
            .iter()
            .filter_map(|i| serde_json::to_string(i).ok())
            .collect();
    }
    if lines.len() >= HISTORY_MAX {
        lines.drain(..lines.len() - HISTORY_MAX + 1);
    }
    if let Ok(line) = serde_json::to_string(item) {
        lines.push(line);
        crate::util::write_atomic(&path, &(lines.join("\n") + "\n"));
    }
}

thread_local! {
    /// In-memory history (newest first), shown in the day window.
    static HISTORY: std::cell::RefCell<Vec<Item>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Last issued notification id.
    static NEXT_ID: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

/// Newest-first snapshot of the notification history, filtered by
/// the configured retention so the day window never shows entries
/// that aged out while the file wasn't written to.
pub fn history() -> Vec<Item> {
    let items = HISTORY.with(|h| h.borrow().clone());
    match history_cutoff() {
        Some(cut) => items.into_iter().filter(|i| i.ts >= cut).collect(),
        None => items,
    }
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
    // Hand the connection to reminder so cards can emit
    // NotificationClosed when they expire on their own.
    card::set_conn(conn);
    let info = gio::DBusNodeInfo::for_xml(XML).unwrap();
    let iface = info
        .lookup_interface("org.freedesktop.Notifications")
        .unwrap();
    let _ = conn
        .register_object("/org/freedesktop/Notifications", &iface)
        .method_call(|_c, _s, _p, _i, method, params, inv| dispatch(method, params, inv))
        .build();
}

/// Shared notification core: allocate an id (or reuse `replaces`),
/// store the item in the in-memory + persisted history and slide
/// the card in. The D-Bus Notify handler and the local watchers
/// (watch.rs) share this so cards, history and id semantics are
/// identical no matter where the event came from.
fn announce(
    cat: card::Category,
    app: &str,
    icon: Option<&gio::Icon>,
    summary: &str,
    body: &str,
    replaces: u32,
) -> u32 {
    let id = if replaces != 0 {
        replaces
    } else {
        NEXT_ID.with(|n| {
            let v = n.get();
            n.set(v + 1);
            v
        })
    };
    let now = glib::DateTime::now_local().ok();
    let item = Item {
        app: app.to_string(),
        icon: icon
            .and_then(|i| i.to_string())
            .map(|g| g.to_string())
            .unwrap_or_default(),
        summary: summary.to_string(),
        body: body.to_string(),
        when: now
            .as_ref()
            .and_then(|d| d.format("%x %H:%M").ok())
            .map(|g| g.to_string())
            .unwrap_or_default(),
        ts: now.as_ref().map(|d| d.to_unix()).unwrap_or_default(),
    };
    append_history(&item);
    HISTORY.with(|h| {
        h.borrow_mut().insert(0, item);
        h.borrow_mut().truncate(HISTORY_MAX);
    });
    card::slide_in_notification(id, cat, icon, summary, body);
    id
}

/// Entry point for Raven's own producers (watch.rs hardware and
/// update events). Same pipeline as bus notifications — app name
/// marks them as locally generated.
pub fn announce_local(cat: card::Category, icon: Option<&gio::Icon>, title: &str, body: &str) {
    announce(cat, "ragnar-raven", icon, title, body, 0);
}

/// Route one method call to its handler.
fn dispatch(method: &str, params: glib::Variant, inv: gio::DBusMethodInvocation) {
    match method {
        "GetCapabilities" => {
            let caps: Vec<String> = ["body", "body-markup", "icon-static", "persistence"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            inv.return_value(Some(&(caps,).to_variant()));
        }
        "GetServerInformation" => {
            inv.return_value(Some(
                &("ragnar-raven", "LieraOS", env!("CARGO_PKG_VERSION"), "1.2").to_variant(),
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
            let Some((app, replaces, icon, summary, body, _a, _h, _t)) = p else {
                inv.return_dbus_error(
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "bad Notify signature",
                );
                return;
            };
            let gicon = if icon.is_empty() {
                None
            } else {
                gio::Icon::for_string(&icon).ok()
            };
            let id = announce(
                card::Category::Default,
                &app,
                gicon.as_ref(),
                &summary,
                &body,
                replaces,
            );
            inv.return_value(Some(&(id,).to_variant()));
        }
        // Spec-correct: close the tracked card (emitting
        // NotificationClosed reason 3 inside close_cards). Unknown
        // ids are simply no-ops.
        "CloseNotification" => {
            if let Some((id,)) = params.get::<(u32,)>() {
                card::close_cards(id);
            }
            inv.return_value(None);
        }
        _ => inv.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
    }
}
