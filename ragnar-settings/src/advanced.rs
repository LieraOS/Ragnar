// Copyright (C) Jörg Schröder
//! Advanced mode: the global switch between the curated normal
//! view and the full depth of settings.
//!
//! Persisted in `~/.config/ragnar-settings/settings.ini` under
//! `[ui] advanced` — app-local state, deliberately not a gsettings
//! schema: no other component consumes it, and the file rides the
//! same `panels::ini` mechanism the panels already use.
//!
//! Two levels consume the flag:
//! - tiles whose registry id is listed in
//!   `registry::catalog::ADVANCED_TILES` stay hidden entirely
//! - panels guard deep rows/sections with [`enabled`] at build
//!   time (panels rebuild on every open, so no live machinery
//!   is needed — toggling takes effect on the next open)

use std::path::PathBuf;

/// Path of the app-local settings file.
fn settings_path() -> PathBuf {
    crate::panels::ini::config_path("ragnar-settings").join("settings.ini")
}

/// Whether advanced mode is on. Missing file/key means off — the
/// default must be the non-overwhelming view.
pub fn enabled() -> bool {
    let map = crate::panels::ini::read(&settings_path());
    crate::panels::ini::get(&map, "ui", "advanced")
        .map(|v| matches!(v.trim(), "true" | "1"))
        .unwrap_or(false)
}

/// Persist the flag, preserving every other line in the file.
pub fn set(on: bool) {
    let _ = crate::panels::ini::write_key(
        &settings_path(),
        "ui",
        "advanced",
        if on { "true" } else { "false" },
    );
}
