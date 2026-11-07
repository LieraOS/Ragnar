//! ragnar-keyring — session keyring starter and unlock-policy helper.
//!
//! gnome-keyring's stock autostart entries carry
//! `OnlyShowIn=GNOME;Unity;MATE`, so under Ragnar the daemon never
//! starts and the Secret Service is missing until something
//! dbus-activates it. With autologin, PAM never sees a password, so
//! the login keyring stays locked and every first secret access pops
//! an unlock dialog.
//!
//! This helper fixes both sides:
//!  1. starts `gnome-keyring-daemon` for the session (secrets+pkcs11),
//!  2. when `auto-unlock` is enabled (default), feeds an EMPTY
//!     password to `gnome-keyring-daemon --unlock`. On systems where
//!     `login.keyring` does not exist yet, gnome-keyring creates an
//!     unencrypted keyring with an empty master password — matching
//!     the silent behaviour users know from other desktops (and from
//!     Chromium's plaintext fallback). If the keyring already has a
//!     real password, the unlock attempt simply fails and the normal
//!     unlock prompt appears — nothing is broken.
//!
//! The policy lives in `~/.config/ragnar-keyring/settings.ini`:
//! ```ini
//! [keyring]
//! auto-unlock=true
//! ```
//! ragnar-settings (Privacy panel) writes the same file, so the user
//! controls the trade-off themselves.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Location of the per-user policy file.
fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/ragnar-keyring/settings.ini")
}

/// Reads `auto-unlock` from the settings file; defaults to true so a
/// fresh system never nags for a keyring password.
fn auto_unlock_enabled() -> bool {
    match std::fs::read_to_string(settings_path()) {
        Ok(text) => !text
            .lines()
            .any(|l| l.trim() == "auto-unlock=false"),
        Err(_) => true,
    }
}

/// Spawns the daemon for this session and applies the unlock policy.
fn main() {
    if auto_unlock_enabled() {
        // Order matters: `--unlock` must run FIRST. It starts the
        // daemon with the login keyring already loaded and unlocked;
        // feeding an empty password creates an unencrypted keyring on
        // fresh systems. Starting `--components` first spawns a
        // keyring-less daemon that never exposes its collections —
        // the bug that left /org/freedesktop/secrets/collection/login
        // missing entirely.
        let mut child = match Command::new("gnome-keyring-daemon")
            .args(["-r", "-d", "--unlock"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ragnar-keyring: unlock spawn failed: {e}");
                return;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            // Bare newline = empty master password.
            let _ = stdin.write_all(b"\n");
        }
        match child.wait() {
            Ok(s) => eprintln!("ragnar-keyring: unlock+start exited {s}"),
            Err(e) => eprintln!("ragnar-keyring: unlock wait failed: {e}"),
        }
    } else {
        eprintln!("ragnar-keyring: auto-unlock disabled by user policy");
        // Still bring the daemon up — locked, prompting on demand.
        let _ = Command::new("gnome-keyring-daemon")
            .args(["-r", "-d"])
            .stdin(Stdio::null())
            .status();
    }

    // Attach the remaining components to the now-running daemon.
    let status = Command::new("gnome-keyring-daemon")
        .args(["--start", "--components=secrets,pkcs11"])
        .stdin(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => eprintln!("ragnar-keyring: components attached"),
        Ok(s) => eprintln!("ragnar-keyring: components attach exited {s}"),
        Err(e) => eprintln!("ragnar-keyring: cannot run components start: {e}"),
    }
}
