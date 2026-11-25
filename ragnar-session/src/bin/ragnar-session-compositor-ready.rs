// Copyright (C) Jörg Schröder
//! ragnar-session-compositor-ready — wait until the Wayland
//! compositor accepts client connections.
//!
//! Replaces the old C helper shipped by the gnome-session fork
//! package. The session startup script gates portal/service
//! startup on this tool: it must only exit 0 once the Wayland
//! socket actually accepts connections, otherwise GTK clients
//! block on D-Bus activation timeouts during startup.
//!
//! A plain Unix-socket connect is sufficient as the gate: the
//! compositor creates and listens on the socket only once it is
//! ready to accept clients. No wayland-client dependency needed.

use std::env;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Total time to wait for the compositor before giving up.
const TIMEOUT: Duration = Duration::from_secs(15);
/// Delay between connection attempts.
const RETRY: Duration = Duration::from_millis(50);

/// Resolves the socket path of the Wayland display.
fn socket_path() -> Option<PathBuf> {
    let display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    if display.starts_with('/') {
        return Some(PathBuf::from(display));
    }
    env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|dir| PathBuf::from(dir).join(display))
}

fn main() {
    let path = match socket_path() {
        Some(p) => p,
        None => {
            // No runtime dir — compositor cannot be up yet; fail so the
            // caller knows gating is impossible rather than racing ahead.
            eprintln!("compositor-ready: XDG_RUNTIME_DIR not set");
            std::process::exit(1);
        }
    };

    let deadline = Instant::now() + TIMEOUT;
    loop {
        match UnixStream::connect(&path) {
            Ok(_) => std::process::exit(0),
            Err(e)
                if Instant::now() < deadline
                    && matches!(
                        e.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
            {
                std::thread::sleep(RETRY);
            }
            Err(e) => {
                eprintln!(
                    "compositor-ready: cannot connect to {}: {e}",
                    path.display()
                );
                std::process::exit(1);
            }
        }
    }
}
