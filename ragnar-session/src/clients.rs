//! Registered session clients and their `ClientPrivate` objects.
//!
//! Every `RegisterClient` call creates a client object at
//! `/org/gnome/SessionManager/ClientN`. The object exposes the
//! `org.gnome.SessionManager.ClientPrivate` interface: the manager
//! emits `QueryEndSession`/`EndSession`/`Stop` signals on it, and the
//! client answers via `EndSessionResponse`.
//!
//! This module keeps the registry; the zbus object interfaces are
//! served from `dbus_sm.rs` where the connection lives.

use std::collections::HashMap;
use zbus::zvariant::OwnedObjectPath;

/// One client registered via `RegisterClient`.
#[derive(Debug)]
pub struct Client {
    /// Sequential client number used in the object path.
    pub index: u32,
    /// Unique client instance ID assigned by the manager.
    pub client_id: String,
    /// Object path under which the client is exported.
    pub object_path: OwnedObjectPath,
}

/// Registry of all registered clients.
#[derive(Default)]
pub struct ClientRegistry {
    next_index: u32,
    entries: HashMap<u32, Client>,
}

impl ClientRegistry {
    /// Registers a client and returns its assigned client ID and
    /// object path.
    pub fn register(&mut self, app_id: String, bus_name: String) -> (String, OwnedObjectPath) {
        self.next_index += 1;
        let index = self.next_index;
        let client_id = uuid_like(index);
        let object_path = OwnedObjectPath::try_from(format!(
            "/org/gnome/SessionManager/Client{index}"
        ))
        .expect("client object path is valid");
        self.entries.insert(
            index,
            Client {
                index,
                client_id: client_id.clone(),
                object_path: object_path.clone(),
            },
        );
        (client_id, object_path)
    }

    /// Removes the client with the given client ID.
    pub fn unregister(&mut self, client_id: &str) -> bool {
        if let Some(idx) = self
            .entries
            .iter()
            .find(|(_, c)| c.client_id == client_id)
            .map(|(i, _)| *i)
        {
            self.entries.remove(&idx);
            true
        } else {
            false
        }
    }

    /// Object paths of registered clients in registration order.
    pub fn sorted_paths(&self) -> Vec<OwnedObjectPath> {
        let mut v: Vec<_> = self.entries.values().collect();
        v.sort_by_key(|c| c.index);
        v.into_iter().map(|c| c.object_path.clone()).collect()
    }
}

/// Generates a unique client ID in the gnome-session style
/// (hex string; uniqueness via timestamp + counter).
fn uuid_like(index: u32) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}{index:x}")
}
