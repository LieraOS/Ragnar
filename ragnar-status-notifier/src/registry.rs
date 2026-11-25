// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-status-notifier — item/host registration logic
//
// The behavioural core: which names to watch, when items count as
// registered/unregistered, and which signals fire — ported 1:1 from
// the legacy FreedesktopWatcher so existing hosts/items behave the same.

use crate::state::{self, ServiceInfo};
use gio::prelude::*;

const FD_IFACE: &str = "org.freedesktop.StatusNotifierWatcher";
const KDE_IFACE: &str = "org.kde.StatusNotifierWatcher";
const FD_PATH: &str = "/org/freedesktop/StatusNotifierWatcher";
const KDE_PATH: &str = "/org/kde/StatusNotifierWatcher";
const BASIC_PATH: &str = "/StatusNotifierWatcher";

/// Handles a RegisterStatusNotifierItem call. `service` is either a
/// bus name or an object path (then the sender owns the item).
pub fn register_item(conn: &gio::DBusConnection, service: &str, sender: &str) {
    let (name, object_path) = if service.starts_with('/') {
        (sender.to_string(), service.to_string())
    } else if let Some(i) = service.rfind(':') {
        // Clients may pass back our own "name:path" entries — split
        // them instead of storing the doubled string as a name.
        if service[i + 1..].starts_with('/') {
            (service[..i].to_string(), service[i + 1..].to_string())
        } else {
            (
                service.to_string(),
                discover_path(conn, service).unwrap_or_else(|| "/StatusNotifierItem".into()),
            )
        }
    } else {
        (
            service.to_string(),
            discover_path(conn, service).unwrap_or_else(|| "/StatusNotifierItem".into()),
        )
    };
    // Same service already watched — ignore like the legacy code did.
    let already = state::with(|r| !r.watched.insert(name.clone()));
    if already {
        return;
    }
    let sender = sender.to_string();
    gio::bus_watch_name(
        gio::BusType::Session,
        &name,
        gio::BusNameWatcherFlags::NONE,
        {
            let name = name.clone();
            let object_path = object_path.clone();
            let sender = sender.clone();
            move |conn, _, owner| {
                let key = format!("{object_path}{sender}{name}");
                let inserted = state::with(|r| {
                    if r.items.contains_key(&key) {
                        false
                    } else {
                        r.items.insert(
                            key,
                            ServiceInfo {
                                name: name.clone(),
                                object_path: object_path.clone(),
                                sender: sender.clone(),
                                owner: owner.to_string(),
                            },
                        );
                        true
                    }
                });
                if inserted {
                    emit_registered(&conn, &name, &object_path, &sender, owner);
                }
            }
        },
        {
            let name = name.clone();
            let object_path = object_path.clone();
            let sender = sender.clone();
            move |conn, _| {
                let key = format!("{object_path}{sender}{name}");
                let removed = state::with(|r| r.items.remove(&key));
                if let Some(info) = removed {
                    emit_unregistered(&conn, &info);
                }
            }
        },
    );
}

/// Handles a RegisterStatusNotifierHost call — watches the host name
/// and updates IsStatusNotifierHostRegistered.
pub fn register_host(service: &str) {
    let service = service.to_string();
    state::with(|r| r.hosts.insert(service.clone()));
    gio::bus_watch_name(
        gio::BusType::Session,
        &service,
        gio::BusNameWatcherFlags::NONE,
        {
            let service = service.clone();
            move |conn, _, _| {
                // Host name gained an owner: the flag may go true and
                // hosts get the spec signal.
                state::with(|r| r.live_hosts.insert(service.clone()));
                emit_all(&conn, "StatusNotifierHostRegistered", None);
            }
        },
        {
            let service = service.clone();
            move |_, _| {
                state::with(|r| {
                    r.live_hosts.remove(&service);
                    r.hosts.remove(&service);
                });
            }
        },
    );
}

/// Whether any watched host currently has an owner — the
/// IsStatusNotifierHostRegistered property. Same semantics as the
/// legacy code: registration alone does not set the flag.
pub fn host_registered() -> bool {
    state::with(|r| !r.live_hosts.is_empty())
}

/// Finds where a name-registered item actually exports its
/// StatusNotifierItem object — some clients (blueman) register
/// their bus name while the object sits at a custom path
/// (/org/blueman/sni). Probes the standard path first, then does a
/// bounded object-tree walk looking for the SNI interface.
fn discover_path(conn: &gio::DBusConnection, name: &str) -> Option<String> {
    if sni_here(conn, name, "/StatusNotifierItem") {
        return Some("/StatusNotifierItem".into());
    }
    // Bounded BFS over the introspection tree — depth 4 covers every
    // sane layout (org/vendor/leaf) without hanging on odd trees.
    let mut todo = vec!["/".to_string()];
    for _ in 0..4 {
        let mut next = Vec::new();
        for path in todo {
            let xml = introspect(conn, name, &path)?;
            for child in node_names(&xml) {
                let p = if path == "/" {
                    format!("/{child}")
                } else {
                    format!("{path}/{child}")
                };
                if sni_here(conn, name, &p) {
                    return Some(p);
                }
                next.push(p);
            }
        }
        todo = next;
    }
    None
}

/// True when the object at `path` answers the SNI interface.
fn sni_here(conn: &gio::DBusConnection, name: &str, path: &str) -> bool {
    let res = conn.call_sync(
        Some(name),
        path,
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&("org.kde.StatusNotifierItem", "Id").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        500,
        None::<&gio::Cancellable>,
    );
    res.is_ok()
}

/// org.freedesktop.DBus.Introspectable.Introspect — XML or None.
fn introspect(conn: &gio::DBusConnection, name: &str, path: &str) -> Option<String> {
    let res = conn.call_sync(
        Some(name),
        path,
        "org.freedesktop.DBus.Introspectable",
        "Introspect",
        None,
        Some(glib::VariantTy::STRING),
        gio::DBusCallFlags::NONE,
        500,
        None::<&gio::Cancellable>,
    );
    res.ok().and_then(|v| v.child_value(0).get::<String>())
}

/// Immediate child node names from an introspection XML blob.
fn node_names(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<node name=\"") {
        rest = &rest[i + 12..];
        if let Some(j) = rest.find('"') {
            out.push(rest[..j].to_string());
        }
    }
    out
}

/// Emits the item-registered signals on both watcher interfaces
/// plus the ragnar extension on the fd.o interface.
fn emit_registered(
    conn: &gio::DBusConnection,
    name: &str,
    object_path: &str,
    sender: &str,
    owner: &str,
) {
    // Entry format "name:path" — consumers need the object path for
    // items whose SNI object does not live at /StatusNotifierItem
    // (e.g. blueman at /org/blueman/sni); a bare name loses it.
    emit_all(
        conn,
        "StatusNotifierItemRegistered",
        Some(&format!("{name}:{object_path}").to_variant()),
    );
    // Ragnar extension carries the full ServiceInfo tuple.
    for path in [FD_PATH, BASIC_PATH] {
        let _ = conn.emit_signal(
            None,
            path,
            FD_IFACE,
            "StatusNotifierItemRegisteredRagnar",
            Some(&glib::Variant::tuple_from_iter([
                name.to_variant(),
                object_path.to_variant(),
                sender.to_variant(),
                owner.to_variant(),
            ])),
        );
    }
}

/// Emits the item-unregistered signals on both interfaces plus the
/// ragnar extension (name, object_path, sender).
fn emit_unregistered(conn: &gio::DBusConnection, info: &ServiceInfo) {
    emit_all(
        conn,
        "StatusNotifierItemUnregistered",
        Some(&info.name.to_variant()),
    );
    for path in [FD_PATH, BASIC_PATH] {
        let _ = conn.emit_signal(
            None,
            path,
            FD_IFACE,
            "StatusNotifierItemUnregisteredRagnar",
            Some(&glib::Variant::tuple_from_iter([
                info.name.to_variant(),
                info.object_path.to_variant(),
                info.sender.to_variant(),
            ])),
        );
    }
}

/// Emits a spec signal on both watcher interfaces and both object
/// paths — hosts may subscribe to either flavour.
fn emit_all(conn: &gio::DBusConnection, signal: &str, arg: Option<&glib::Variant>) {
    let payload = match arg {
        Some(v) => glib::Variant::tuple_from_iter([v.clone()]),
        None => glib::Variant::tuple_from_iter(std::iter::empty::<glib::Variant>()),
    };
    for (iface, path) in [
        (FD_IFACE, FD_PATH),
        (FD_IFACE, BASIC_PATH),
        (KDE_IFACE, KDE_PATH),
        (KDE_IFACE, BASIC_PATH),
    ] {
        let _ = conn.emit_signal(None, path, iface, signal, Some(&payload));
    }
}
