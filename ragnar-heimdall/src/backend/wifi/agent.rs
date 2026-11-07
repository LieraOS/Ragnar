//! iwd passphrase agent. During `Network.Connect` iwd calls back into
//! the registering client when the network has no stored credentials —
//! `RequestPassphrase` then hands over the passphrase the user typed.
//! After a successful join iwd writes `/var/lib/iwd/<ssid>.psk` itself,
//! so credentials persist without any privileged helper.

use std::sync::Mutex;

use anyhow::{Context, Result};
use zbus::blocking::Connection;
use zbus::fdo;
use zbus::zvariant::OwnedObjectPath;

use super::{AGENT_MGR_IFACE, ROOT_PATH, SERVICE};

/// Object path our agent is exported on.
const AGENT_PATH: &str = "/org/lieraos/ragnar_heimdall/agent";

/// Shared slot for the passphrase of the in-flight connect.
/// Filled by `connect()` before registration and read by the agent
/// when iwd asks for credentials.
pub struct PassphraseAgent {
    passphrase: Mutex<Option<String>>,
}

impl PassphraseAgent {
    pub fn new(passphrase: Option<String>) -> Self {
        Self {
            passphrase: Mutex::new(passphrase),
        }
    }
}

/// `net.connman.iwd.Agent` — iwd calls `RequestPassphrase` when the
/// target network lacks stored credentials. The EAP/enterprise
/// callbacks are intentionally unsupported: returning an error aborts
/// that exchange, which is the correct behaviour for a PSK-only UI.
#[zbus::interface(name = "net.connman.iwd.Agent")]
impl PassphraseAgent {
    async fn release(&mut self) {}

    fn request_passphrase(&self, _network: OwnedObjectPath) -> fdo::Result<String> {
        self.passphrase
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .ok_or_else(|| fdo::Error::Failed("no passphrase provided".into()))
    }

    fn request_private_key_passphrase(&self, _network: OwnedObjectPath) -> fdo::Result<String> {
        Err(fdo::Error::NotSupported(
            "enterprise EAP unsupported".into(),
        ))
    }

    fn request_user_name_and_password(
        &self,
        _network: OwnedObjectPath,
    ) -> fdo::Result<(String, String)> {
        Err(fdo::Error::NotSupported(
            "enterprise EAP unsupported".into(),
        ))
    }

    fn request_user_password(&self, _network: OwnedObjectPath) -> fdo::Result<String> {
        Err(fdo::Error::NotSupported(
            "enterprise EAP unsupported".into(),
        ))
    }

    fn cancel(&mut self, _reason: String) {}
}

/// Export the agent object and register it with iwd's AgentManager.
/// The caller keeps the connection alive for the duration of the
/// connect; dropping the connection un-exports the object, and iwd
/// tolerates agents that vanish mid-operation.
pub fn register_agent(conn: &Connection, passphrase: Option<String>) -> Result<()> {
    let agent = PassphraseAgent::new(passphrase);
    conn.object_server()
        .at(AGENT_PATH, agent)
        .context("export passphrase agent")?;

    let mgr = zbus::blocking::Proxy::new(conn, SERVICE, ROOT_PATH, AGENT_MGR_IFACE)
        .context("iwd agent manager proxy")?;
    let path: OwnedObjectPath = AGENT_PATH.try_into().context("agent object path")?;
    mgr.call_method("RegisterAgent", &(path,))
        .context("register passphrase agent with iwd")?;
    Ok(())
}

/// Best-effort cleanup — iwd forgets vanished agents anyway, so a
/// failure here is harmless.
pub fn unregister_agent(conn: &Connection) {
    let Ok(mgr) = zbus::blocking::Proxy::new(conn, SERVICE, ROOT_PATH, AGENT_MGR_IFACE) else {
        return;
    };
    if let Ok(path) = OwnedObjectPath::try_from(AGENT_PATH) {
        let _ = mgr.call_method("UnregisterAgent", &(path,));
    }
}
