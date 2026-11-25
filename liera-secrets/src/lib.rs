// Copyright (C) Jörg Schröder
//! Shared cloud-credential store for LieraOS applications.
//!
//! Backed by the freedesktop Secret Service API
//! (org.freedesktop.secrets) — gnome-keyring, KeePassXC or any
//! conforming daemon does the actual encryption; this crate only
//! speaks the standard D-Bus protocol, so every consumer
//! (ragnar-raven CalDAV sync, Valkyrie WebDAV mounts, ragnar-settings
//! account editor) stays replaceable and shares one credential
//! namespace: `liera-cloud`.

use dbus_secret_service::{EncryptionType, SecretService};
use std::collections::HashMap;

/// One stored cloud account. `service` is the namespace key the
/// account is filed under (e.g. `nextcloud`, `webdav-files`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudAccount {
    pub service: String,
    pub url: String,
    pub user: String,
    pub password: String,
}

/// Attribute namespace inside the keyring so Liera items never
/// collide with foreign items.
const ATTR_SERVICE: &str = "liera-cloud-service";
const ATTR_URL: &str = "liera-cloud-url";
const ATTR_USER: &str = "liera-cloud-user";
const LABEL_PREFIX: &str = "LieraOS Cloud";

fn connect() -> Result<SecretService, String> {
    SecretService::connect(EncryptionType::Dh)
        .map_err(|e| format!("secret service connect failed: {e}"))
}

/// Store or replace an account. `replace = true` keeps exactly one
/// item per service name.
pub fn store(account: &CloudAccount) -> Result<(), String> {
    let ss = connect()?;
    let col = ss
        .get_default_collection()
        .map_err(|e| format!("no default collection: {e}"))?;
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_SERVICE, account.service.as_str());
    attrs.insert(ATTR_URL, account.url.as_str());
    attrs.insert(ATTR_USER, account.user.as_str());
    col.create_item(
        &format!("{LABEL_PREFIX}: {}", account.service),
        attrs,
        account.password.as_bytes(),
        true, // replace existing item for the same service
        "text/plain",
    )
    .map_err(|e| format!("create_item failed: {e}"))?;
    Ok(())
}

/// Pick the first usable item: unlocked preferred, otherwise unlock
/// the first locked one (prompts the keyring once if needed).
fn first_item<'a>(
    res: dbus_secret_service::SearchItemsResult<dbus_secret_service::Item<'a>>,
) -> Result<Option<dbus_secret_service::Item<'a>>, String> {
    if let Some(it) = res.unlocked.into_iter().next() {
        return Ok(Some(it));
    }
    match res.locked.into_iter().next() {
        Some(it) => {
            it.unlock().map_err(|e| format!("unlock failed: {e}"))?;
            Ok(Some(it))
        }
        None => Ok(None),
    }
}

/// Look up the account stored under `service`, if any.
pub fn get(service: &str) -> Result<Option<CloudAccount>, String> {
    let ss = connect()?;
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_SERVICE, service);
    let res = ss
        .search_items(attrs)
        .map_err(|e| format!("search failed: {e}"))?;
    let Some(item) = first_item(res)? else {
        return Ok(None);
    };
    let secret = item
        .get_secret()
        .map_err(|e| format!("get_secret failed: {e}"))?;
    let a = item
        .get_attributes()
        .map_err(|e| format!("get_attributes failed: {e}"))?;
    Ok(Some(CloudAccount {
        service: service.into(),
        url: a.get(ATTR_URL).cloned().unwrap_or_default(),
        user: a.get(ATTR_USER).cloned().unwrap_or_default(),
        password: String::from_utf8_lossy(&secret).into_owned(),
    }))
}

/// Remove the account stored under `service`. Missing = ok.
pub fn delete(service: &str) -> Result<(), String> {
    let ss = connect()?;
    let mut attrs = HashMap::new();
    attrs.insert(ATTR_SERVICE, service);
    let res = ss
        .search_items(attrs)
        .map_err(|e| format!("search failed: {e}"))?;
    for it in res.unlocked.iter().chain(res.locked.iter()) {
        it.delete().map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok(())
}

/// List service names of all stored cloud accounts (no secrets).
pub fn list() -> Result<Vec<String>, String> {
    let ss = connect()?;
    let col = ss
        .get_default_collection()
        .map_err(|e| format!("no default collection: {e}"))?;
    let items = col
        .get_all_items()
        .map_err(|e| format!("list failed: {e}"))?;
    let mut out = Vec::new();
    for it in items {
        if let Ok(a) = it.get_attributes() {
            if let Some(s) = a.get(ATTR_SERVICE) {
                out.push(s.clone());
            }
        }
    }
    Ok(out)
}
