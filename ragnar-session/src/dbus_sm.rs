// Copyright (C) Jörg Schröder
//! The `org.gnome.SessionManager` D-Bus interface.
//!
//! Exposes the subset of the GNOME SessionManager contract that the
//! Ragnar session actually consumes: client registration with the
//! `ClientPrivate` handshake, end-session orchestration via the
//! `EndSessionDialog` contract, inhibition tracking, session
//! properties and the `Initialized`/`SessionOver` signals.

use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::zvariant::OwnedObjectPath;

use crate::clients::ClientRegistry;
use crate::inhibit::InhibitorRegistry;

/// Shared session state behind the D-Bus interfaces.
#[derive(Default)]
pub struct SessionState {
    /// Registered clients (RegisterClient objects).
    pub clients: ClientRegistry,
    /// Active inhibition requests.
    pub inhibitors: InhibitorRegistry,
    /// Session name taken from the `.session` file.
    pub session_name: String,
    /// True once required components have been spawned.
    pub running: bool,
}

/// The `org.gnome.SessionManager` interface implementation.
pub struct SessionManager {
    /// Shared registry/state; also drives the logout orchestrator.
    pub state: Arc<Mutex<SessionState>>,
    /// Channel that triggers the end-session flow in `logout.rs`.
    /// Payload: (action flags, caller-provided log-out mode).
    pub end_session_tx: tokio::sync::mpsc::Sender<EndRequest>,
}

/// One end-session request queued by a D-Bus method.
pub struct EndRequest {
    /// 0 = Logout, 1 = Reboot, 2 = Shutdown (internal mapping).
    pub action: u32,
    /// gnome-session logout mode (0 normal, 1 no-confirm, 2 force).
    pub mode: u32,
}

#[zbus::interface(name = "org.gnome.SessionManager")]
impl SessionManager {
    /// Registers a client; returns its object path.
    async fn register_client(
        &self,
        app_id: &str,
        _client_startup_id: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let bus_name = header
            .sender()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default();
        let mut st = self.state.lock().await;
        let (client_id, path) = st.clients.register(app_id.to_string(), bus_name.clone());
        // Serve Client + ClientPrivate on the new path so the client
        // receives signals and can call EndSessionResponse.
        conn.object_server()
            .at(&path, crate::client_obj::ClientInfo {
                client_id: client_id.clone(),
                app_id: app_id.to_string(),
                bus_name,
            })
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        conn.object_server()
            .at(&path, crate::client_obj::ClientPrivate {
                client_id,
                object_path: path.to_string(),
            })
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(path)
    }

    /// Removes a client by its assigned client ID.
    async fn unregister_client(&self, client_id: &str) {
        self.state.lock().await.clients.unregister(client_id);
    }

    /// Object paths of all registered clients.
    async fn get_clients(&self) -> Vec<OwnedObjectPath> {
        self.state.lock().await.clients.sorted_paths()
    }

    /// Object paths of all inhibitors (each /InhibitorN is a stub —
    /// callers only use the count/flags, so paths are enumerated).
    async fn get_inhibitors(&self) -> Vec<OwnedObjectPath> {
        // Inhibitor detail objects are out of scope; returning an
        // empty list matches gnome-session when nothing is inhibited.
        Vec::new()
    }

    /// Queues a logout (or end-session dialog for `mode == 0`).
    async fn logout(&self, mode: u32) {
        let _ = self
            .end_session_tx
            .send(EndRequest { action: 0, mode })
            .await;
    }

    /// Queues a reboot via the same end-session pipeline.
    async fn reboot(&self) {
        let _ = self
            .end_session_tx
            .send(EndRequest { action: 1, mode: 0 })
            .await;
    }

    /// Queues a shutdown via the same end-session pipeline.
    async fn shutdown(&self) {
        let _ = self
            .end_session_tx
            .send(EndRequest { action: 2, mode: 0 })
            .await;
    }

    /// Registers an inhibitor; returns its cookie.
    async fn inhibit(
        &self,
        _app_id: &str,
        _toplevel_xid: u32,
        reason: &str,
        flags: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<u32> {
        let bus_name = header
            .sender()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default();
        let mut st = self.state.lock().await;
        Ok(st.inhibitors.add(bus_name, reason.to_string(), flags))
    }

    /// Removes an inhibitor by cookie.
    async fn uninhibit(&self, cookie: u32) {
        self.state.lock().await.inhibitors.remove(cookie);
    }

    /// Whether any inhibitor blocks the given action flags.
    async fn is_inhibited(&self, flags: u32) -> bool {
        self.state.lock().await.inhibitors.is_inhibited(flags)
    }

    /// Whether the session has finished starting.
    async fn is_session_running(&self) -> bool {
        self.state.lock().await.running
    }

    /// Environment propagation stub (systemd user env owns this).
    async fn setenv(&self, variable: &str, value: &str) {
        std::env::set_var(variable, value);
    }

    /// Locale lookup; index 0 = LANG, others return empty.
    async fn get_locale(&self, category: i32) -> String {
        if category == 0 {
            std::env::var("LANG").unwrap_or_default()
        } else {
            String::new()
        }
    }

    /// Emitted once all required components have been launched.
    #[zbus(signal)]
    async fn initialized(ctxt: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;

    /// Emitted when the session is ending (before exit).
    #[zbus(signal)]
    async fn session_over(ctxt: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;

    /// Session name property.
    #[zbus(property)]
    async fn session_name(&self) -> String {
        self.state.lock().await.session_name.clone()
    }

    /// The session is always active (no seat switching in Ragnar).
    #[zbus(property)]
    async fn session_is_active(&self) -> bool {
        true
    }

    /// Compositor/renderer identification string.
    #[zbus(property)]
    async fn renderer(&self) -> String {
        "Ragnar/labwc".to_string()
    }

    /// Bitmask of currently inhibited actions.
    #[zbus(property)]
    async fn inhibited_actions(&self) -> u32 {
        self.state.lock().await.inhibitors.inhibited_actions()
    }
}
