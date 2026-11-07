//! End-session orchestration.
//!
//! Mirrors the gnome-session logout pipeline: signal `QueryEndSession`
//! to registered clients, optionally open the `EndSessionDialog`
//! (served by ragnar-power-dialog) for user confirmation, then emit
//! `EndSession`, wait for responses, emit `Stop`, emit `SessionOver`
//! and finally exit — rebooting or powering off via logind when the
//! action requires it.

use zbus::Connection;

use crate::dbus_sm::EndRequest;

/// Runs one queued end-session request to completion.
///
/// `action`: 0 = logout, 1 = reboot, 2 = shutdown.
/// `mode`:  0 = show confirmation dialog, 1/2 = proceed directly.
pub async fn run(conn: &Connection, req: EndRequest) -> zbus::Result<()> {
    // 1. Ask clients whether the session may end (QueryEndSession).
    broadcast_client_signal(conn, "QueryEndSession", 1u32).await;

    // 2. Confirmation dialog for interactive logouts.
    if req.mode == 0 {
        let dialog_type: u32 = req.action; // 0 logout, 1 reboot, 2 shutdown
        let confirmed = open_end_session_dialog(conn, dialog_type).await?;
        if !confirmed {
            // User cancelled — leave the session running.
            broadcast_client_signal(conn, "EndSession", 0u32).await;
            return Ok(());
        }
    }

    // 3. Tell clients to save state and quit (EndSession, then Stop).
    broadcast_client_signal(conn, "EndSession", 1u32).await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    broadcast_client_signal(conn, "Stop", 0u32).await;

    // 4. SessionOver for anything still listening, then act.
    let _ = conn
        .emit_signal(
            None::<&str>,
            "/org/gnome/SessionManager",
            "org.gnome.SessionManager",
            "SessionOver",
            &(),
        )
        .await;

    match req.action {
        1 => logind(conn, "Reboot").await?,
        2 => logind(conn, "PowerOff").await?,
        _ => {}
    }
    std::process::exit(0);
}

/// Emits a `ClientPrivate` signal on every registered client object.
async fn broadcast_client_signal(conn: &Connection, member: &str, flags: u32) {
    let st_paths = match conn
        .call_method(
            Some("org.gnome.SessionManager"),
            "/org/gnome/SessionManager",
            Some("org.gnome.SessionManager"),
            "GetClients",
            &(),
        )
        .await
    {
        Ok(m) => m
            .body()
            .deserialize::<Vec<zbus::zvariant::OwnedObjectPath>>()
            .unwrap_or_default(),
        Err(_) => return,
    };
    for path in st_paths {
        let _ = conn
            .emit_signal(
                None::<&str>,
                path,
                "org.gnome.SessionManager.ClientPrivate",
                member,
                &flags,
            )
            .await;
    }
}

/// Calls `Open` on the EndSessionDialog and awaits a `Confirmed*`
/// signal matching the dialog type; returns false on `Closed`/timeout.
async fn open_end_session_dialog(conn: &Connection, dialog_type: u32) -> zbus::Result<bool> {
    use futures_util::StreamExt;
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.gnome.SessionManager.EndSessionDialog")?
        .build();
    let mut stream = zbus::MessageStream::for_match_rule(rule, conn, None).await?;

    conn.call_method(
        Some("org.gnome.SessionManager"),
        "/org/gnome/SessionManager/EndSessionDialog",
        Some("org.gnome.SessionManager.EndSessionDialog"),
        "Open",
        &(dialog_type, 0u32, 0u32, 0u32),
    )
    .await?;

    let want = match dialog_type {
        1 => "ConfirmedReboot",
        2 => "ConfirmedShutdown",
        _ => "ConfirmedLogout",
    };
    let wait = async {
        while let Some(msg) = stream.next().await {
            if let Ok(m) = msg {
                if m.header().member().map(|n| n.as_str()) == Some(want) {
                    return true;
                }
                if m.header().member().map(|n| n.as_str()) == Some("Closed") {
                    return false;
                }
            }
        }
        false
    };
    Ok(tokio::time::timeout(std::time::Duration::from_secs(60), wait)
        .await
        .unwrap_or(false))
}

/// Invokes a logind power action on the system bus.
async fn logind(session: &Connection, method: &str) -> zbus::Result<()> {
    let sys = Connection::system().await?;
    let _ = session;
    sys.call_method(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        Some("org.freedesktop.login1.Manager"),
        method,
        &true,
    )
    .await?;
    Ok(())
}
