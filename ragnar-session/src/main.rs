// Copyright (C) Jörg Schröder
//! ragnar-session — Rust replacement for the gnome-session fork.
//!
//! Responsibilities (verified against the live SessionManager API):
//!  * own `org.gnome.SessionManager` on the session bus,
//!  * launch `.session` RequiredComponents with AutoRestart,
//!  * serve the RegisterClient/ClientPrivate contract,
//!  * orchestrate logout/reboot/shutdown via EndSessionDialog,
//!  * keep systemd's xdg-autostart-generator handling of
//!    `/etc/xdg/autostart` untouched (no autostart phase here).

mod client_obj;
mod clients;
mod components;
mod dbus_sm;
mod inhibit;
mod logout;
mod session_file;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> zbus::Result<()> {
    // --version exits before any session work starts.
    if std::env::args().any(|a| a == "--version") {
        println!("ragnar-session {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // Locate the .session file: --session=<name> or default Desktop.
    let arg = std::env::args()
        .find(|a| a.starts_with("--session="))
        .map(|a| a.trim_start_matches("--session=").to_string())
        .unwrap_or_else(|| "Desktop".to_string());
    // RAGNAR_SESSION_FILE overrides the lookup for standalone tests.
    let session_path = std::env::var("RAGNAR_SESSION_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            // GNOME-compatible location — shipped by ragnar-core.
            std::path::PathBuf::from(format!(
                "/usr/share/gnome-session/sessions/{arg}.session"
            ))
        });
    let session = match session_file::SessionFile::load(&session_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "ragnar-session: cannot read {}: {e} — no required components!",
                session_path.display()
            );
            session_file::SessionFile {
                name: arg.clone(),
                required: Vec::new(),
            }
        }
    };
    eprintln!(
        "ragnar-session: starting session '{}' ({} required components)",
        session.name,
        session.required.len()
    );

    let state = Arc::new(Mutex::new(dbus_sm::SessionState {
        session_name: session.name.clone(),
        ..Default::default()
    }));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<dbus_sm::EndRequest>(4);
    let iface = dbus_sm::SessionManager {
        state: state.clone(),
        end_session_tx: tx,
    };

    let conn = zbus::connection::Builder::session()?
        .name("org.gnome.SessionManager")?
        .serve_at("/org/gnome/SessionManager", iface)?
        .build()
        .await?;

    // Launch required components and monitor them for restarts.
    let (spawn_tx, mut spawn_rx) = tokio::sync::mpsc::channel::<(String, String, bool)>(8);
    let mut children: Vec<(String, bool, tokio::process::Child)> = Vec::new();
    for comp in &session.required {
        if let Some(path) = session_file::find_desktop_file(&comp.id) {
            if let Some((exec, restart)) = session_file::parse_desktop_exec(&path) {
                match components::spawn(&exec) {
                    Ok(child) => children.push((comp.id.clone(), restart, child)),
                    Err(e) => eprintln!("ragnar-session: {} spawn failed: {e}", comp.id),
                }
            }
        } else {
            eprintln!("ragnar-session: no desktop file for {}", comp.id);
        }
    }
    {
        state.lock().await.running = true;
    }
    let _ = conn
        .emit_signal(
            None::<&str>,
            "/org/gnome/SessionManager",
            "org.gnome.SessionManager",
            "Initialized",
            &(),
        )
        .await;

    // Activate the session target: the generated app-*@autostart.service
    // units hang off xdg-desktop-autostart.target, which refuses manual
    // starts and may only be pulled in by dependency. ragnar-session.target
    // (mirroring cinnamon-session.target) declares those dependencies, so
    // starting it brings up graphical-session{,-pre}.target and the whole
    // autostart tree.
    // The target can survive a login inside the user manager — a
    // second `start` on an already-active unit is then a no-op and
    // the autostart tree would never be re-pulled (the second-login
    // black screen). Stopping a bare target is instant (targets own
    // no processes); without PropagatesStopTo nothing cascades.
    let _ = tokio::process::Command::new("systemctl")
        .args(["--user", "stop", "ragnar-session.target"])
        .output()
        .await;
    match tokio::process::Command::new("systemctl")
        .args(["--user", "start", "ragnar-session.target"])
        .output()
        .await
    {
        Ok(out) if out.status.success() => {
            eprintln!("ragnar-session: started ragnar-session.target")
        }
        Ok(out) => eprintln!(
            "ragnar-session: session target failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => eprintln!("ragnar-session: systemctl failed: {e}"),
    }

    // Supervision loop: respawn AutoRestart components, forward
    // end-session requests to the orchestrator.
    loop {
        tokio::select! {
            Some(req) = rx.recv() => {
                let c = conn.clone();
                tokio::spawn(async move {
                    if let Err(e) = logout::run(&c, req).await {
                        eprintln!("ragnar-session: end-session failed: {e}");
                    }
                });
            }
            _ = poll_children(&mut children, &spawn_tx) => {
                while let Ok((id, exec, restart)) = spawn_rx.try_recv() {
                    if restart {
                        if let Ok(child) = components::spawn(&exec) {
                            children.push((id, true, child));
                        }
                    }
                }
            }
        }
    }
}

/// Checks all supervised children for exits and queues restarts.
async fn poll_children(
    children: &mut Vec<(String, bool, tokio::process::Child)>,
    tx: &tokio::sync::mpsc::Sender<(String, String, bool)>,
) {
    use futures_util::future::join_all;
    let futs = children.iter_mut().enumerate().map(|(i, (_, restart, child))| {
        let restart = *restart;
        async move { (i, child.try_wait().ok().flatten(), restart) }
    });
    let results = join_all(futs).await;
    let mut dead = Vec::new();
    for (i, status, restart) in results {
        if status.is_some() {
            dead.push((i, restart));
        }
    }
    for (i, restart) in dead.into_iter().rev() {
        let (id, _, _) = children.remove(i);
        eprintln!("ragnar-session: component {id} exited (restart={restart})");
        if restart {
            // Re-resolve the Exec line at respawn time.
            if let Some(p) = session_file::find_desktop_file(&id) {
                if let Some((exec, r)) = session_file::parse_desktop_exec(&p) {
                    let _ = tx.send((id, exec, r)).await;
                }
            }
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}
