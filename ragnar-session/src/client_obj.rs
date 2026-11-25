// Copyright (C) Jörg Schröder
//! Per-client D-Bus objects served under
//! `/org/gnome/SessionManager/ClientN`.
//!
//! Two interfaces live on each client path:
//!  * `org.gnome.SessionManager.Client` — read-only client info
//!    (ID, app ID, D-Bus name, PID, status),
//!  * `org.gnome.SessionManager.ClientPrivate` — the handshake the
//!    power dialog uses: the manager emits `QueryEndSession`,
//!    `EndSession` and `Stop`; the client calls
//!    `EndSessionResponse` to acknowledge.

/// Client info interface (`org.gnome.SessionManager.Client`).
pub struct ClientInfo {
    /// Manager-assigned unique client ID.
    pub client_id: String,
    /// Application ID supplied at registration.
    pub app_id: String,
    /// Unique D-Bus name of the owning connection.
    pub bus_name: String,
}

#[zbus::interface(name = "org.gnome.SessionManager.Client")]
impl ClientInfo {
    /// Manager-assigned client ID.
    fn get_id(&self) -> String {
        self.client_id.clone()
    }

    /// Application ID registered by the client.
    fn get_app_id(&self) -> String {
        self.app_id.clone()
    }

    /// Startup ID (unused — always empty in Ragnar).
    fn get_startup_id(&self) -> String {
        String::new()
    }

    /// Restart style: 0 = RestartIfRunning (gnome-session default).
    fn get_restart_style_hint(&self) -> u32 {
        0
    }

    /// PID of the owning connection (0 when resolvable only
    /// through the bus — kept simple for now).
    fn get_unix_process_id(&self) -> u32 {
        0
    }

    /// Client status: 0 = running/idle.
    fn get_status(&self) -> u32 {
        0
    }

    /// Unique D-Bus name of the client connection.
    fn get_dbus_name(&self) -> String {
        self.bus_name.clone()
    }
}

/// Private handshake interface
/// (`org.gnome.SessionManager.ClientPrivate`).
///
/// Methods are invoked by the client; signals are emitted by the
/// manager on this object path during end-session.
pub struct ClientPrivate {
    /// Manager-assigned client ID (for logging/diagnostics).
    pub client_id: String,
    /// Object path this interface is served on.
    pub object_path: String,
}

#[zbus::interface(name = "org.gnome.SessionManager.ClientPrivate")]
impl ClientPrivate {
    /// The client's answer to `EndSession`/`QueryEndSession`.
    /// `is_ok` means the client is ready for the session to end.
    fn end_session_response(&self, is_ok: bool, reason: &str) {
        eprintln!(
            "ragnar-session: client {} EndSessionResponse ok={} reason='{}'",
            self.client_id, is_ok, reason
        );
    }

    /// PID lookup via the bus (left to GetUnixProcessId).
    fn get_pid(&self) -> u32 {
        0
    }

    /// UID of the client process owner.
    fn get_unix_user(&self) -> u32 {
        0
    }

    /// Log state transition helper required by the contract.
    fn set_status(&self, _status: u32) {}
}
