// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-display-svc — XWayland RandR integration
// Copyright (C) 2026 LieraOS Project
//
// Sets the RandR primary output on the XWayland server. Wayland has no
// primary-output concept, but X11 clients (Steam, Proton games, legacy
// apps) query RandR to decide which monitor is "primary" — e.g. where
// fullscreen windows should open. XWayland exposes outputs with their
// real connector names, so the same connector can be set directly.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Retry cadence while XWayland is not up yet. XWayland starts lazily
/// with the first X11 client — possibly minutes after the session (and
/// this service) came up — so the loop keeps trying until it succeeds.
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// Generation counter: a newer set_primary() call supersedes any older
/// still-retrying thread, so the last requested primary always wins.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Mark the given connector as the RandR primary output on XWayland.
/// `connector` is the output connector name (e.g. "DP-3").
///
/// Returns immediately: the retry loop runs on a background thread so
/// the D-Bus service stays responsive. The compositor hint file is
/// written right away — it does not depend on XWayland.
pub fn set_primary(connector: &str) {
    let connector = connector.to_string();
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        // The hint file is for labwc and works without XWayland.
        write_primary_hint(&connector);
        loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                // A newer primary was selected — this thread is stale.
                return;
            }
            match Command::new("xrandr")
                .args(["--output", &connector, "--primary"])
                .output()
            {
                Ok(out) if out.status.success() => {
                    eprintln!("ragnar-display-svc: xrandr primary -> {}", connector);
                    return;
                }
                _ => std::thread::sleep(RETRY_DELAY),
            }
        }
    });
}

/// Write the primary connector name to a session file that the
/// compositor reads when placing X11 windows. wlroots has no
/// primary-output concept and XWayland's RandR primary lives inside
/// the XWayland process, unreachable for the compositor — so the file
/// is the channel. Written atomically; labwc ignores a missing file
/// entirely (fallback = stock behaviour).
fn write_primary_hint(connector: &str) {
    // Runtime dir is always set inside the user session.
    let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") else {
        return;
    };
    let dir = format!("{rt}/ragnar");
    let _ = std::fs::create_dir_all(&dir);
    let tmp = format!("{}/xwayland-primary.tmp", dir);
    let dst = format!("{}/xwayland-primary", dir);
    if std::fs::write(&tmp, connector).is_ok() {
        let _ = std::fs::rename(&tmp, &dst);
    }
}
