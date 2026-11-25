// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-status-notifier — the D-Bus surface
//
// Owns org.freedesktop.StatusNotifierWatcher and
// org.kde.StatusNotifierWatcher (plus the daemon's own
// StatusNotifierHost name, same bootstrap order as the legacy service).
// Both watchers expose the same object implementation on their
// canonical path and /StatusNotifierWatcher — clients pick either.

use crate::{registry, state};
use gio::prelude::*;

const FD_NAME: &str = "org.freedesktop.StatusNotifierWatcher";
const KDE_NAME: &str = "org.kde.StatusNotifierWatcher";
const HOST_NAME: &str = "org.freedesktop.StatusNotifierHost-ragnar_daemon";
const FD_PATH: &str = "/org/freedesktop/StatusNotifierWatcher";
const KDE_PATH: &str = "/org/kde/StatusNotifierWatcher";
const BASIC_PATH: &str = "/StatusNotifierWatcher";

/// Introspection for one watcher interface — `iface` decides the
/// D-Bus interface name; the contract is otherwise identical.
fn xml(iface: &str) -> String {
    format!(
        r#"<node>
  <interface name="{iface}">
    <method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg type="s" direction="in"/></method>
    <method name="GetRegisteredStatusNotifierPathnamesRagnar"><arg type="a(ssss)" direction="out"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
    <signal name="StatusNotifierItemRegisteredRagnar"><arg type="s"/><arg type="s"/><arg type="s"/><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregisteredRagnar"><arg type="s"/><arg type="s"/><arg type="s"/></signal>
  </interface>
</node>"#
    )
}

/// Owns all bus names in the same order the legacy service did:
/// host name first, then fd.o watcher, plus the kde watcher.
pub fn own_names() -> Vec<gio::OwnerId> {
    let mut ids = Vec::new();
    ids.push(gio::bus_own_name(
        gio::BusType::Session,
        HOST_NAME,
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| {
            // Register ourselves as host first — mirrors the legacy
            // bootstrap which self-registered before owning the
            // watcher name.
            registry::register_host(HOST_NAME);
            register(&conn, FD_NAME);
        },
        |_, _| {},
        |_, _| eprintln!("ragnar-status-notifier: lost {HOST_NAME}"),
    ));
    ids.push(gio::bus_own_name(
        gio::BusType::Session,
        FD_NAME,
        gio::BusNameOwnerFlags::NONE,
        |conn, _| register(&conn, FD_NAME),
        |_, _| {},
        |_, _| eprintln!("ragnar-status-notifier: lost {FD_NAME}"),
    ));
    ids.push(gio::bus_own_name(
        gio::BusType::Session,
        KDE_NAME,
        gio::BusNameOwnerFlags::NONE,
        |conn, _| register(&conn, KDE_NAME),
        |_, _| {},
        |_, _| eprintln!("ragnar-status-notifier: lost {KDE_NAME}"),
    ));
    ids
}

/// Registers the watcher object on both its canonical path and the
/// legacy /StatusNotifierWatcher path.
fn register(conn: &gio::DBusConnection, iface: &str) {
    let node = gio::DBusNodeInfo::for_xml(&xml(iface)).expect("valid xml");
    let Some(info) = node.lookup_interface(iface) else {
        return;
    };
    for path in [FD_PATH, KDE_PATH, BASIC_PATH] {
        // Only register paths matching this interface's own
        // canonical path, plus the shared basic path.
        if path == FD_PATH && iface != FD_NAME {
            continue;
        }
        if path == KDE_PATH && iface != KDE_NAME {
            continue;
        }
        conn.register_object(path, &info)
            .method_call(|conn, sender, _, _, method, params, inv| {
                match method {
                    "RegisterStatusNotifierItem" => {
                        if let Some(svc) = params.child_value(0).get::<String>() {
                            registry::register_item(&conn, &svc, sender.unwrap_or_default());
                        }
                    }
                    "RegisterStatusNotifierHost" => {
                        if let Some(host) = params.child_value(0).get::<String>() {
                            registry::register_host(&host);
                        }
                    }
                    // Ragnar extension: the full registered tuples.
                    "GetRegisteredStatusNotifierPathnamesRagnar" => {
                        let items = state::with(|r| {
                            r.items
                                .values()
                                .map(|i| {
                                    (
                                        i.name.clone(),
                                        i.object_path.clone(),
                                        i.sender.clone(),
                                        i.owner.clone(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        });
                        inv.return_value(Some(&glib::Variant::tuple_from_iter([
                            items.to_variant()
                        ])));
                        return;
                    }
                    _ => {}
                }
                inv.return_value(None);
            })
            .property(|_, _, _, _, prop| match prop {
                "RegisteredStatusNotifierItems" => state::item_names().to_variant(),
                "IsStatusNotifierHostRegistered" => registry::host_registered().to_variant(),
                "ProtocolVersion" => 0i32.to_variant(),
                _ => 0i32.to_variant(),
            })
            .build();
    }
}
