// Copyright (C) Jörg Schröder
// The StatusNotifierWatcher itself: owns org.kde.StatusNotifierWatcher
// on the session bus, answers RegisterStatusNotifierItem calls, pulls
// each item's properties and re-fetches on change/death signals.
use crate::tray::{Tray, TrayItem};
use gtk4::gio;
use gtk4::prelude::*;

const IFACE_XML: &str = r"<node>
  <interface name='org.kde.StatusNotifierWatcher'>
    <method name='RegisterStatusNotifierItem'>
      <arg name='service' type='s' direction='in'/>
    </method>
    <method name='RegisterStatusNotifierHost'>
      <arg name='service' type='s' direction='in'/>
    </method>
    <property name='RegisteredStatusNotifierItems' type='as' access='read'/>
    <property name='IsStatusNotifierHostRegistered' type='b' access='read'/>
    <signal name='StatusNotifierItemRegistered'>
      <arg name='service' type='s'/>
    </signal>
    <signal name='StatusNotifierItemUnregistered'>
      <arg name='service' type='s'/>
    </signal>
  </interface>
</node>";

/// Run the watcher inside the glib main loop.
pub async fn run(conn: gio::DBusConnection, tray: Tray, tx: async_channel::Sender<()>) {
    let node = gio::DBusNodeInfo::for_xml(IFACE_XML).expect("watcher XML");
    let info = node.interfaces().first().unwrap();

    // Kept alive for the process lifetime — registrations die with it.
    let keepalive = std::rc::Rc::new(std::cell::RefCell::new(
        Vec::<gio::SignalSubscriptionId>::new(),
    ));

    {
        let (tray_m, tx_m, conn_m, _keep_m) =
            (tray.clone(), tx.clone(), conn.clone(), keepalive.clone());
        let _reg = conn
            .register_object("/StatusNotifierWatcher", info)
            .method_call(
                move |_conn, sender, _obj, _iface, method, params, invocation| {
                    if method == "RegisterStatusNotifierItem" {
                        let service = params.get::<(String,)>().map(|p| p.0).unwrap_or_default();
                        let sender = sender.unwrap_or_default().to_string();
                        let (bus, path) = if service.starts_with('/') {
                            (sender, service)
                        } else {
                            (service, "/StatusNotifierItem".to_string())
                        };
                        register_item(&conn_m, &tray_m, &tx_m, bus, path);
                    }
                    invocation.return_value(None);
                },
            )
            .property({
                let tray = tray.clone();
                move |_conn, _sender, _obj, _iface, name| match name {
                    "RegisteredStatusNotifierItems" => tray
                        .items
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|i| i.key.clone())
                        .collect::<Vec<_>>()
                        .to_variant(),
                    _ => true.to_variant(), // IsStatusNotifierHostRegistered
                }
            })
            .build();
        let _ = _reg;
    }

    // Try to own the watcher name — usually already held by a session
    // service (ragnar-daemon today). Either way we also register as a
    // HOST: panels are hosts, the watcher is a shared session service.
    // Direct FFI call with NULL callbacks: gio's own_closure unwraps the
    // name argument as &str, and GLib may pass NULL on name_lost — that
    // panicked across FFI and killed the taskbar (gio dbus.rs own_closure).
    use gtk4::glib::translate::{IntoGlib, ToGlibPtr};
    let _owner = unsafe {
        gio::ffi::g_bus_own_name_on_connection(
            conn.to_glib_none().0,
            "org.kde.StatusNotifierWatcher".to_glib_none().0,
            gio::BusNameOwnerFlags::NONE.into_glib(),
            None,
            None,
            std::ptr::null_mut(),
            None,
        )
    };

    // Register ourselves as a host so the watcher holds us. Async —
    // a slow/absent watcher must not stall the main loop (this runs
    // inside the tray future on the GTK context).
    let _ = conn
        .call_future(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "RegisterStatusNotifierHost",
            Some(&("org.lieraos.ragnar.Taskbar",).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            2000,
        )
        .await;

    // React to new items the foreign watcher announces: the signal
    // only carries the service string, so re-read the full list.
    let (tray_s, tx_s, conn_s) = (tray.clone(), tx.clone(), conn.clone());
    keepalive.borrow_mut().push(conn.signal_subscribe(
        None,
        Some("org.kde.StatusNotifierWatcher"),
        Some("StatusNotifierItemRegistered"),
        Some("/StatusNotifierWatcher"),
        None,
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, _| {
            refresh_from_watcher(&conn_s, &tray_s, &tx_s);
        },
    ));

    // The watcher name may have no owner yet at login (ragnar-daemon
    // starts after the bar): the initial Get below then answers empty
    // and early autostart items would stay invisible until the next
    // registration signal. Re-sweep on every owner change instead.
    let (tray_n, tx_n, conn_n) = (tray.clone(), tx.clone(), conn.clone());
    keepalive.borrow_mut().push(conn.signal_subscribe(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some("org.kde.StatusNotifierWatcher"),
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, _| {
            refresh_from_watcher(&conn_n, &tray_n, &tx_n);
        },
    ));

    // Initial sweep: items that registered before we started.
    refresh_from_watcher(&conn, &tray, &tx);

    // Keep the future alive — callbacks drive everything.
    std::future::pending::<()>().await;
}

/// Re-read RegisteredStatusNotifierItems from whichever watcher owns
/// the name and register everything not yet in the table.
fn refresh_from_watcher(conn: &gio::DBusConnection, tray: &Tray, tx: &async_channel::Sender<()>) {
    let (tray2, tx2, conn2) = (tray.clone(), tx.clone(), conn.clone());
    conn.call(
        Some("org.kde.StatusNotifierWatcher"),
        "/StatusNotifierWatcher",
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(
            &(
                "org.kde.StatusNotifierWatcher",
                "RegisteredStatusNotifierItems",
            )
                .to_variant(),
        ),
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            let Ok(v) = res else { return };
            // (v) -> variant -> the array itself
            let sv = v.child_value(0).child_value(0);
            let Some(list) = sv.get::<Vec<String>>() else {
                return;
            };
            for entry in list {
                // Entries are "bus:path" or bare bus names — rfind so
                // unique names like ":1.74:/org/x/y" split correctly
                // (first ':' would hit the name's own separator).
                let (bus, path) = if let Some(i) = entry.rfind(':') {
                    if entry[i + 1..].starts_with('/') {
                        (entry[..i].to_string(), entry[i + 1..].to_string())
                    } else {
                        (entry.clone(), "/StatusNotifierItem".to_string())
                    }
                } else {
                    (entry.clone(), "/StatusNotifierItem".to_string())
                };
                register_item(&conn2, &tray2, &tx2, bus, path);
            }
        },
    );
}

/// Register a newly announced item: resolve path vs. sender, fetch its
/// initial properties, and arm signal watches for icon/status changes
/// plus a name-vanished cleanup.
fn register_item(
    conn: &gio::DBusConnection,
    tray: &Tray,
    tx: &async_channel::Sender<()>,
    bus: String,
    path: String,
) {
    let sender = bus.clone();
    let key = format!("{bus}:{path}");
    {
        let mut items = tray.items.lock().unwrap();
        if items.iter().any(|i| i.key == key) {
            return;
        }
        items.push(TrayItem {
            key: key.clone(),
            sender: bus.clone(),
            path: path.clone(),
            icon_name: String::new(),
            icon_theme_path: String::new(),
            menu_path: String::new(),
            icon_pixmap: None,
            status: "Active".into(),
        });
    }
    // Notify right away — a failed property fetch must not leave the
    // slot invisible.
    let _ = tx.try_send(());

    // Fetch icon/status now, then refetch on any item signal.
    super::item::fetch(conn, tray, tx, bus.clone(), path.clone());
    let (tray2, tx2, conn2) = (tray.clone(), tx.clone(), conn.clone());
    let (bus2, path2) = (bus.clone(), path.clone());
    let sig = conn.signal_subscribe(
        Some(&bus),
        Some("org.kde.StatusNotifierItem"),
        None,
        Some(&path),
        None,
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, _| {
            super::item::fetch(&conn2, &tray2, &tx2, bus2.clone(), path2.clone());
        },
    );
    let _ = sig; // subscription lives on the connection

    // Drop the item when its owner leaves the bus.
    let (tray3, tx3) = (tray.clone(), tx.clone());
    let sender2 = sender.clone();
    let _watch = gio::bus_watch_name_on_connection(
        conn,
        &sender,
        gio::BusNameWatcherFlags::NONE,
        |_, _, _| {},
        move |_, _| {
            tray3
                .items
                .lock()
                .unwrap()
                .retain(|i| !i.key.starts_with(&format!("{sender2}:")));
            let _ = tx3.try_send(());
        },
    );
}
