// Copyright (C) Jörg Schröder
// System-tray applet backend — a StatusNotifierWatcher on the session
// bus. Applications (Discord, Steam, Nextcloud, Blueman, …) register
// their StatusNotifierItem objects here; the watcher tracks them,
// pulls icon/status properties and streams snapshots to the GTK loop.
pub mod dbusmenu;
mod dbusmenu_parse;
mod dbusmenu_ui;
mod item;
mod watcher;

use gtk4::glib;
use std::sync::{Arc, Mutex};

/// One registered tray item as the applet renders it.
#[derive(Clone)]
pub struct TrayItem {
    /// "sender:path" — unique key + activation target.
    pub key: String,
    /// Bus name owning the item (":1.x" or a well-known name).
    pub sender: String,
    /// Object path of the item on the sender's connection.
    pub path: String,
    /// Themed icon name from the item (empty if pixmap-only).
    pub icon_name: String,
    /// Item-supplied icon search dir (IconThemePath) — ayatana apps
    /// ship private icon sets like "status_icon_0" there.
    pub icon_theme_path: String,
    /// Object path of the item's com.canonical.dbusmenu object.
    pub menu_path: String,
    /// Raw ARGB32 pixmap (w, h, bytes) when the item ships pixels.
    pub icon_pixmap: Option<(i32, i32, Vec<u8>)>,
    /// "Active" | "Passive" | "NeedsAttention" — Passive items hide.
    pub status: String,
}

/// Shared item table: the D-Bus side writes, the GTK side reads the
/// latest snapshot. Snapshots also flow through the notify channel so
/// the applet only redraws on real change.
#[derive(Clone)]
pub struct Tray {
    pub items: Arc<Mutex<Vec<TrayItem>>>,
}

/// Start the watcher on the session bus; returns the tray store and
/// a receiver that fires one unit per snapshot update.
pub fn start() -> (Tray, async_channel::Receiver<()>) {
    let (tx, rx) = async_channel::unbounded::<()>();
    let tray = Tray {
        items: Arc::new(Mutex::new(Vec::new())),
    };
    let t = tray.clone();
    glib::spawn_future_local(async move {
        let Ok(conn) = gtk4::gio::bus_get_future(gtk4::gio::BusType::Session).await else {
            return;
        };
        watcher::run(conn, t, tx).await;
    });
    (tray, rx)
}
