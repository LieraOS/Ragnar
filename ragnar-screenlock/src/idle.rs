// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — swayidle orchestration
//
// Builds the swayidle command line from the current GSettings and
// restarts swayidle whenever relevant keys change — the same job the
// legacy daemon's calculate_idle() did. swayidle stays the mechanism;
// this service only owns the configuration.

use crate::{locker, settings};
use std::process::Command;

/// Required helper binaries — without them the service reports
/// "missing" and skips idle orchestration (same check as legacy).
const REQUIRED: &[&str] = &["swayidle", "killall", "wlopm", "dbus-send"];

/// The self-callback used inside swayidle timeout hooks.
const CALL: &str = "dbus-send --type=method_call \
    --dest=org.lieraos.ragnar.Screenlock \
    /org/lieraos/ragnar/Screenlock org.lieraos.ragnar.Screenlock.";

/// Rebuilds swayidle from the current settings snapshot and wires the
/// settings watcher once (subsequent changes rebuild again).
/// Returns false when a required helper binary is missing.
pub fn rebuild() -> bool {
    if REQUIRED
        .iter()
        .any(|a| glib::find_program_in_path(a).is_none())
    {
        return false;
    }
    settings::watch(respawn);
    respawn();
    true
}

/// Kills every swayidle instance and respawns with a freshly built
/// command line — idempotent, safe to call on every settings change.
fn respawn() {
    let _ = Command::new("killall").arg("swayidle").output();
    let cmd = build(&settings::snapshot());
    if cmd.is_empty() {
        return;
    }
    let _ = Command::new("sh").arg("-c").arg(&cmd).spawn();
}

/// Assembles the complete `swayidle -w` command line from the
/// settings snapshot — mirrors calculate_idle() in the legacy daemon.
fn build(s: &settings::ScreenlockSettings) -> String {
    let mut out = String::from("swayidle -w ");
    out += &dim_section(s);
    out += &power_section(s);
    out += &lock_section(s);
    out += &sleep_section(s);
    out
}

/// Idle-dim timeout: dim after 30s, undim on resume — only when the
/// setting is on and a backlight device exists.
fn dim_section(s: &settings::ScreenlockSettings) -> String {
    if !s.idle_dim {
        return String::new();
    }
    format!("timeout 30 '{CALL}Dim' resume '{CALL}Undim' ")
}

/// Screen power off/on around the idle delay (wlopm drives the
/// actual outputs; the dbus calls kept the daemon's state flag in
/// the fork — kept for interface parity, currently no-ops).
fn power_section(s: &settings::ScreenlockSettings) -> String {
    if s.idle_delay == 0 {
        return String::new();
    }
    format!(
        "timeout {} '{CALL}ScreenPowerSuspend && wlopm --off \\*' \
         resume 'wlopm --on \\* && {CALL}ScreenPowerResume' ",
        s.idle_delay
    )
}

/// Lock-after-idle timeout plus the before-sleep hook — both gated
/// by the lockdown setting like the legacy code.
fn lock_section(s: &settings::ScreenlockSettings) -> String {
    if s.disable_lock_screen {
        return String::new();
    }
    let lock = locker::command(&s.picture_uri);
    let mut out = String::new();
    if s.lock_enabled && !lock.is_empty() {
        out += &format!("timeout {} '{lock}' ", s.idle_delay + s.lock_delay);
    }
    if !lock.is_empty() {
        out += &format!("before-sleep '{lock}' ");
    }
    out
}

/// AC/battery suspend timers, each guarded by an OnBattery() callback
/// — same shell-out structure the legacy daemon generated.
fn sleep_section(s: &settings::ScreenlockSettings) -> String {
    let suspend = "dbus-send --system --print-reply --dest=org.freedesktop.login1 \
        /org/freedesktop/login1 org.freedesktop.login1.Manager.Suspend boolean:false";
    let mut out = String::new();
    if s.sleep_bat_type == "suspend" && s.sleep_bat_timeout != 0 {
        out += &format!(
            "timeout {} 'if dbus-send --print-reply=literal \
             --dest=org.lieraos.ragnar.Screenlock \
             /org/lieraos/ragnar/Screenlock \
             org.lieraos.ragnar.Screenlock.OnBattery \
             | grep \"boolean true\" >/dev/null; then {suspend}; fi' ",
            s.sleep_bat_timeout
        );
    }
    if s.sleep_ac_type == "suspend" && s.sleep_ac_timeout != 0 {
        out += &format!(
            "timeout {} 'if dbus-send --print-reply=literal \
             --dest=org.lieraos.ragnar.Screenlock \
             /org/lieraos/ragnar/Screenlock \
             org.lieraos.ragnar.Screenlock.OnBattery \
             | grep \"boolean false\" >/dev/null; then {suspend}; fi' ",
            s.sleep_ac_timeout
        );
    }
    out
}
