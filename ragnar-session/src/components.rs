// Copyright (C) Jörg Schröder
//! Launching and supervision of required session components.
//!
//! Each required component is resolved to a desktop entry, spawned
//! once at session start, and — if its desktop file requests
//! `X-GNOME-AutoRestart` — respawned when it exits unexpectedly.

use std::process::Stdio;
use tokio::process::Command;

/// Spawns a desktop-entry Exec line and returns a spawned component
/// together with a tokio child handle for exit monitoring.
///
/// The command runs through a shell because desktop Exec lines may
/// contain argument quoting.
pub fn spawn(exec: &str) -> std::io::Result<tokio::process::Child> {
    Command::new("sh")
        .arg("-c")
        .arg(format!("exec {exec}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
