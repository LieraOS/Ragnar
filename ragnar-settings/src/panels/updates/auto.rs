// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "Updates" panel: privileged async actions
// Copyright (C) 2026 LieraOS Project
//
// Every privileged step runs through pkexec on a worker thread and
// reports back over an async_channel — the GTK main loop never
// blocks on apt or the pkexec password dialog. Shell quoting stays
// safe by passing the managed drop-in as a positional parameter to
// /bin/sh instead of interpolating it into the command string.

use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;

use super::apt;

/// Completion callback invoked on the GTK main thread — it never
/// crosses a thread (only the job's Result travels the channel), so
/// it may capture non-Send GTK widgets.
pub type Done<T> = Box<dyn FnOnce(Result<T, String>) + 'static>;

/// Run `job` on a worker thread, deliver its result via `done` on
/// the main context.
fn run_async<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, String> + Send + 'static,
    done: Done<T>,
) {
    let (tx, rx) = async_channel::unbounded::<Result<T, String>>();
    std::thread::spawn(move || {
        let _ = tx.send_blocking(job());
    });
    gtk4::glib::MainContext::default().spawn_local(async move {
        if let Ok(res) = rx.recv().await {
            done(res);
        }
    });
}

/// Write the managed drop-in via pkexec and (re)enable the apt
/// daily timers — without the timers the feature is dead even with
/// a correct config file. The body is staged to a user-writable
/// temp file first so the shell command needs no quoting tricks.
pub fn apply_auto(enabled: bool, done: Done<()>) {
    run_async(
        move || {
            // Stage in the private runtime dir (0700) with a fresh
            // 0600 file — NEVER a predictable /tmp name: pkexec then
            // installs this file as root into apt.conf.d, and a
            // replaced pre-existing file/symlink would be a local
            // privilege-escalation hole. No fallback to /tmp.
            // g_get_user_runtime_dir always yields a per-user dir
            // (XDG_RUNTIME_DIR or a 0700 fallback) — an empty result
            // is the only error worth handling.
            let runtime_dir = gtk4::glib::user_runtime_dir();
            if runtime_dir.as_os_str().is_empty() {
                return Err("no runtime dir".to_string());
            }
            let tmp = runtime_dir.join(format!("ragnar-52auto-upgrades-{}", std::process::id()));
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .and_then(|mut f| {
                    use std::io::Write;
                    f.write_all(apt::render_dropin(enabled).as_bytes())
                })
                .map_err(|e| e.to_string())?;
            let r = Command::new("pkexec")
                .args([
                    "/bin/sh",
                    "-c",
                    "install -m 0644 \"$1\" \"$2\" && \
                     systemctl enable --now apt-daily.timer apt-daily-upgrade.timer",
                    "sh",
                    &tmp.to_string_lossy(),
                    apt::DROPIN_PATH,
                ])
                .output()
                .map_err(|e| e.to_string());
            let _ = std::fs::remove_file(&tmp);
            check(r?)
        },
        done,
    );
}

/// Install the unattended-upgrades package itself (pkexec).
pub fn install_unattended_upgrades(done: Done<()>) {
    run_async(
        || {
            let r = Command::new("pkexec")
                .args(["apt-get", "install", "-y", "unattended-upgrades"])
                .output()
                .map_err(|e| e.to_string());
            check(r?)
        },
        done,
    );
}

/// Refresh the package lists (pkexec apt-get update).
pub fn refresh_lists(done: Done<()>) {
    run_async(
        || {
            let r = Command::new("pkexec")
                .args(["apt-get", "update"])
                .output()
                .map_err(|e| e.to_string());
            check(r?)
        },
        done,
    );
}

/// Count pending upgrades off-thread (apt simulation takes a moment).
pub fn count_async(done: Done<usize>) {
    run_async(apt::count_updates, done);
}

/// Command output → Result, surfacing the last stderr line so the
/// UI can show a meaningful error (pkexed denial, apt failure).
fn check(out: std::process::Output) -> Result<(), String> {
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error")
            .trim()
            .to_string())
    }
}
