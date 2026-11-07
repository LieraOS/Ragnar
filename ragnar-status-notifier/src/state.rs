// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-status-notifier — the in-memory registry
//
// Mirrors the legacy service: everything lives in memory. Items are
// tracked by a composite key (object_path + sender + name) so two
// different callers can never clobber each other.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// One registered StatusNotifierItem — the same tuple the legacy
/// DBusServiceInfo struct carried and the ragnar signals emit.
#[derive(Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub object_path: String,
    pub sender: String,
    pub owner: String,
}

pub struct Registry {
    /// Registered items, keyed by object_path+sender+name.
    pub items: HashMap<String, ServiceInfo>,
    /// Host names registered for watching.
    pub hosts: HashSet<String>,
    /// Host names that currently have a bus owner — drives the
    /// IsStatusNotifierHostRegistered property.
    pub live_hosts: HashSet<String>,
    /// Item names already watched — prevents double registration.
    pub watched: HashSet<String>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry {
        items: HashMap::new(),
        hosts: HashSet::new(),
        live_hosts: HashSet::new(),
        watched: HashSet::new(),
    });
}

/// Runs `f` with mutable access to the shared registry.
pub fn with<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    REGISTRY.with(|r| f(&mut r.borrow_mut()))
}

/// "name:path" entries of all registered items, for the
/// RegisteredStatusNotifierItems property — the path lets hosts
/// reach items whose SNI object is not at /StatusNotifierItem.
pub fn item_names() -> Vec<String> {
    with(|r| {
        r.items
            .values()
            .map(|i| format!("{}:{}", i.name, i.object_path))
            .collect()
    })
}
