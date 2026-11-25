// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-clipboard — the data-control daemon.
//
// Watches the Wayland clipboard via wlr/ext-data-control: every new
// selection is captured into the on-disk history, and when the
// selection is cleared (e.g. its source app exits) the daemon
// re-provides the newest entry so clipboard contents survive the
// lifetime of the app they were copied from.
//
// A small unix socket accepts commands from the picker/CLI:
//   set <id>   re-set the clipboard to a stored entry
//   clear      wipe history and clear the clipboard
//   ping       liveness check

use crate::store;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use wl_clipboard_rs::copy::{self, MimeSource, MimeType, Source};
use wl_clipboard_rs::paste::Seat;
use wl_clipboard_rs::watch::{self, ClipboardEvent};

/// IPC socket path inside XDG_RUNTIME_DIR.
pub fn socket_path() -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()))
        .join("ragnar-clipboard.sock")
}

/// Set the regular clipboard to the MIME/byte pairs of one entry.
/// Returns when the source is installed; serving runs on a
/// background thread inside the (long-lived) daemon.
fn provide(entry: &store::Entry) -> Result<(), String> {
    let mut sources = Vec::new();
    for (mime, _) in &entry.mimes {
        if let Some(bytes) = store::blob(entry, mime) {
            sources.push(MimeSource {
                source: Source::Bytes(bytes.into_boxed_slice()),
                mime_type: MimeType::Specific(mime.clone()),
            });
        }
    }
    if sources.is_empty() {
        return Err("entry has no readable blobs".into());
    }
    copy::Options::new()
        .copy_multi(sources)
        .map_err(|e| e.to_string())
}

/// Handle one IPC command line. `entries` is the daemon's live
/// history — `clear` must empty it together with the disk store,
/// otherwise the watcher would try to re-provide wiped entries.
fn handle_command(line: &str, entries: &mut Vec<store::Entry>) -> String {
    let mut parts = line.split_whitespace();
    match parts.next() {
        Some("set") => {
            let id = parts.next().unwrap_or("");
            match entries.iter().find(|e| e.id == id) {
                Some(e) => match provide(e) {
                    Ok(()) => "ok".into(),
                    Err(err) => format!("error {err}"),
                },
                None => "error no such id".into(),
            }
        }
        Some("clear") => {
            entries.clear();
            store::wipe();
            let _ = copy::clear(
                copy::ClipboardType::Regular,
                copy::Seat::All,
            );
            "ok".into()
        }
        Some("ping") => "pong".into(),
        _ => "error unknown command".into(),
    }
}

/// Send one command to the running daemon; returns the reply.
pub fn send(cmd: &str) -> Result<String, String> {
    let mut sock = UnixStream::connect(socket_path()).map_err(|e| e.to_string())?;
    writeln!(sock, "{cmd}").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(&sock).read_line(&mut reply).map_err(|e| e.to_string())?;
    Ok(reply.trim().to_string())
}

/// IPC listener thread: forwards commands to the shared entry list.
fn spawn_ipc(entries: std::sync::Arc<std::sync::Mutex<Vec<store::Entry>>>) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ragnar-clipboard: cannot bind {}: {e}", path.display());
            return;
        }
    };
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let reply = {
                let mut line = String::new();
                if BufReader::new(&conn).read_line(&mut line).is_err() {
                    continue;
                }
                let mut guard = entries.lock().unwrap();
                handle_command(line.trim(), &mut guard)
            };
            let mut w = conn;
            let _ = writeln!(w, "{reply}");
        }
    });
}

/// Blocking daemon loop — run in the foreground under the session's
/// autostart.
pub fn run() -> i32 {
    let mut watcher = match watch::Watcher::new(
        watch::ClipboardType::Regular,
        Seat::Unspecified,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ragnar-clipboard: cannot watch clipboard: {e}");
            return 1;
        }
    };

    let entries = std::sync::Arc::new(std::sync::Mutex::new(store::load()));
    spawn_ipc(entries.clone());

    loop {
        match watcher.next_event() {
            Ok(Some(ClipboardEvent::Changed {
                clipboard: _,
                mime_types,
                mut offer,
            })) => {
                if store::is_sensitive(&mime_types) {
                    continue;
                }
                // Receive the bytes of every wanted MIME the source
                // offers — the store keeps whichever arrive.
                let mut captured = Vec::new();
                for mime in &mime_types {
                    if !store::WANTED_MIMES.contains(&mime.as_str()) {
                        continue;
                    }
                    if let Ok(pipe) = offer.receive(mime) {
                        let data = store::read_capped(pipe, 8 * 1024 * 1024);
                        if !data.is_empty() {
                            captured.push((mime.clone(), data));
                        }
                    }
                }
                if let Some(entry) = store::store(captured) {
                    let mut g = entries.lock().unwrap();
                    g.insert(0, entry);
                    g.truncate(store::MAX_ENTRIES);
                }
            }
            Ok(Some(ClipboardEvent::Cleared { .. })) => {
                // Source vanished (app exited or cleared): hand the
                // newest history entry back to the compositor so the
                // content survives. The Changed event our own source
                // triggers is deduplicated by the content hash.
                let newest = entries.lock().unwrap().first().cloned();
                if let Some(e) = newest {
                    if let Err(err) = provide(&e) {
                        eprintln!("ragnar-clipboard: re-provide failed: {err}");
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("ragnar-clipboard: watch error: {e}");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
    0
}
