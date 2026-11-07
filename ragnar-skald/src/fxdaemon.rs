//! `ragnar-skald-fx` — headless effect-chain daemon.
//!
//! Filter-chain modules live in the PipeWire context of whichever
//! process loaded them; to keep the configured effects alive
//! system-wide — regardless of whether the GUI window is open — a tiny
//! background process owns them. This module implements that process.
//!
//! Control flow:
//!
//! ```text
//! GUI writes settings.json ──spawn──▶ fx daemon (Unix-socket lock,
//!        │                            worker thread owns modules)
//!        └── mtime poll ◀──────────── re-reads settings on every change,
//!                                     pushes commands into its worker
//! ```
//!
//! The GUI needs no socket protocol: enable/disable goes through the
//! settings file, and live slider values are written straight onto the
//! (globally visible) fx node `params` — PipeWire accepts control
//! writes from any client.
//!
//! The daemon is permanent audio infrastructure: the virtual mic is
//! the session's default input and must never vanish, so the process
//! runs as a systemd user service (`Restart=always`) and only exits
//! when the PipeWire backend itself is lost.

use std::fs;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, Result};
use tracing::{debug, info, warn};

use crate::audio::config::AudioConfig;
use crate::audio::AudioEffect;
use crate::config::ConfigPersistence;
use crate::pipewire::backend::PwBackend;
use crate::pipewire::types::{BackendEvent, FxEffect, MicEffect};

/// How often the daemon re-checks the settings file for changes.
const WATCH_INTERVAL: Duration = Duration::from_millis(300);

/// Name of the daemon binary next to the GUI executable.
const DAEMON_BIN: &str = "ragnar-skald-fx";

/// Unix socket used purely as a single-instance lock — peers can only
/// *connect* to check liveness; no data is exchanged.
fn socket_path() -> Option<PathBuf> {
    dirs::runtime_dir()
        .or_else(dirs::state_dir)
        .map(|d| d.join("ragnar-skald-fx.sock"))
}

/// `true` when a daemon instance already answers on the lock socket.
pub fn is_running() -> bool {
    socket_path()
        .map(|p| UnixStream::connect(&p).is_ok())
        .unwrap_or(false)
}

/// Spawn the fx daemon unless it is already running. Called by the GUI
/// after the user enables a chain; a second spawn attempt exits
/// immediately through the socket lock, so this is idempotent.
pub fn ensure_running() -> Result<()> {
    if is_running() {
        return Ok(());
    }

    // Prefer the binary next to the running executable (installed
    // layout), fall back to PATH (development tree via cargo run).
    let exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(DAEMON_BIN)))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(DAEMON_BIN));

    Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn fx daemon {exe:?}"))?;
    info!("Spawned fx daemon {exe:?}");
    Ok(())
}

/// Entry point of the `ragnar-skald-fx` binary.
///
/// Returns the process exit code. The daemon idles until every effect
/// chain is disabled in settings, then cleans up and exits.
pub fn run() -> i32 {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let persist = match ConfigPersistence::new() {
        Ok(p) => p,
        Err(e) => {
            warn!("fx daemon: no config persistence: {e:#}");
            return 1;
        }
    };

    let mut cfg = match persist.load() {
        Ok(s) => s.audio_config,
        Err(e) => {
            warn!("fx daemon: settings load failed: {e:#}");
            AudioConfig::default()
        }
    };

    // Single-instance lock: a live socket means another daemon owns it.
    let path = match socket_path() {
        Some(p) => p,
        None => {
            warn!("fx daemon: no runtime dir for lock socket");
            return 1;
        }
    };
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(_) => {
            // Stale socket? Probe — dead sockets refuse a connection.
            if UnixStream::connect(&path).is_ok() {
                debug!("fx daemon already running — exiting");
                return 0;
            }
            let _ = fs::remove_file(&path);
            match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    warn!("fx daemon: cannot bind lock socket: {e}");
                    return 1;
                }
            }
        }
    };
    // Accepting is not required for the lock semantics; drain peers so
    // the backlog does not fill up with GUI liveness probes.
    let _ = listener.set_nonblocking(true);
    thread::spawn(move || loop {
        match listener.accept() {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(_) => break,
        }
    });

    let backend = match PwBackend::start() {
        Ok(b) => b,
        Err(e) => {
            warn!("fx daemon: backend start failed: {e:#}");
            return 1;
        }
    };
    apply_config(&backend, &cfg);
    let mut last_mtime = settings_mtime(&persist);

    loop {
        thread::sleep(WATCH_INTERVAL);

        // Bail out when the PipeWire daemon went away.
        while let Ok(ev) = backend.try_recv_event() {
            if let BackendEvent::BackendLost(m) = ev {
                warn!("fx daemon: backend lost ({m}) — exiting");
                let _ = fs::remove_file(&path);
                return 1;
            }
        }

        let mtime = settings_mtime(&persist);
        if mtime == last_mtime {
            continue;
        }
        last_mtime = mtime;
        match persist.load() {
            Ok(s) => {
                cfg = s.audio_config;
                apply_config(&backend, &cfg);
            }
            Err(e) => warn!("fx daemon: settings reload failed: {e:#}"),
        }
    }
}

/// Push the complete audio config into the daemon's worker. Every
/// command is idempotent; structural changes (preset/gate) only rebuild
/// when the value actually differs.
fn apply_config(backend: &PwBackend, cfg: &AudioConfig) {
    backend.set_fx_enabled(cfg.fx_enabled);
    backend.set_effect_value(FxEffect::Bass, f32::from(cfg.equalizer.bass()));
    backend.set_effect_value(FxEffect::Treble, f32::from(cfg.equalizer.treble()));
    backend.set_effect_value(FxEffect::Echo, f32::from(cfg.echo.intensity()));
    backend.set_effect_value(FxEffect::Reverb, f32::from(cfg.reverb.intensity()));

    // Structural mic settings first so the chain loads with the right
    // graph on the first attempt.
    backend.set_voice_preset(cfg.mic_preset);
    backend.set_mic_gate(cfg.mic_gate);
    backend.set_mic_fx_enabled(cfg.mic_fx_enabled);
    backend.set_mic_effect_value(MicEffect::Denoise, cfg.mic_gate_level);
    backend.set_mic_effect_value(MicEffect::Echo, cfg.mic_echo_level);
    backend.set_mic_effect_value(MicEffect::Voice, cfg.mic_voice_pitch);
}

/// File modification time of `settings.json` — the watch trigger.
fn settings_mtime(persist: &ConfigPersistence) -> Option<SystemTime> {
    fs::metadata(persist.config_path())
        .and_then(|m| m.modified())
        .ok()
}
