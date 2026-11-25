use gio::prelude::*;
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — GSettings readers
//
// The legacy daemon read the same schemas; keep them so existing
// settings (set via gsettings or the old control center) keep
// working unchanged.


use gio::Settings;

/// Power schema: idle dimming and suspend timers.
const SCHEMA_POWER: &str = "org.gnome.settings-daemon.plugins.power";
/// Session schema: global idle delay.
const SCHEMA_SESSION: &str = "org.gnome.desktop.session";
/// Screensaver schema: lock after idle and the wallpaper used by swaylock.
const SCHEMA_SAVER: &str = "org.gnome.desktop.screensaver";
/// Lockdown schema: admin kill-switch for the lock screen.
const SCHEMA_LOCKDOWN: &str = "org.gnome.desktop.lockdown";

/// One settings snapshot for the idle rebuild; every key is read in
/// one place so the idle builder stays declarative.
pub struct ScreenlockSettings {
    /// Seconds of inactivity before the screen is powered off; 0 = off.
    pub idle_delay: u32,
    /// Lock after idle_delay + this many seconds.
    pub lock_delay: u32,
    /// Lock screen enabled at all.
    pub lock_enabled: bool,
    /// Admin lockdown flag — disables locking entirely.
    pub disable_lock_screen: bool,
    /// Dim the screen while idle.
    pub idle_dim: bool,
    /// Percentage to dim to.
    pub idle_brightness: i32,
    /// AC suspend: seconds + action type string.
    pub sleep_ac_timeout: i32,
    pub sleep_ac_type: String,
    /// Battery suspend: seconds + action type string.
    pub sleep_bat_timeout: i32,
    pub sleep_bat_type: String,
    /// Wallpaper URI for the swaylock fallback locker.
    pub picture_uri: String,
}

/// Reads every relevant key once and returns the snapshot.
pub fn snapshot() -> ScreenlockSettings {
    let power = Settings::new(SCHEMA_POWER);
    let session = Settings::new(SCHEMA_SESSION);
    let saver = Settings::new(SCHEMA_SAVER);
    let lockdown = Settings::new(SCHEMA_LOCKDOWN);
    ScreenlockSettings {
        idle_delay: session.uint("idle-delay"),
        lock_delay: saver.uint("lock-delay"),
        lock_enabled: saver.boolean("lock-enabled"),
        disable_lock_screen: lockdown.boolean("disable-lock-screen"),
        idle_dim: power.boolean("idle-dim"),
        idle_brightness: power.int("idle-brightness"),
        sleep_ac_timeout: power.int("sleep-inactive-ac-timeout"),
        sleep_ac_type: power.string("sleep-inactive-ac-type").to_string(),
        sleep_bat_timeout: power.int("sleep-inactive-battery-timeout"),
        sleep_bat_type: power.string("sleep-inactive-battery-type").to_string(),
        picture_uri: saver.string("picture-uri").to_string(),
    }
}

/// Connects change notifications of every relevant key to `cb` —
/// the idle manager re-runs its rebuild on any change.
pub fn watch<F: Fn() + 'static>(cb: F) {
    let cb = std::rc::Rc::new(cb);
    let fire = |s: &Settings, keys: &'static [&'static str]| {
        let cb = cb.clone();
        s.connect_changed(None, move |_, key| {
            if keys.contains(&key) {
                cb();
            }
        });
    };
    fire(
        &Settings::new(SCHEMA_POWER),
        &[
            "sleep-inactive-ac-timeout",
            "sleep-inactive-ac-type",
            "sleep-inactive-battery-timeout",
            "sleep-inactive-battery-type",
            "idle-dim",
            "idle-brightness",
        ],
    );
    fire(&Settings::new(SCHEMA_SESSION), &["idle-delay"]);
    fire(
        &Settings::new(SCHEMA_SAVER),
        &["lock-enabled", "lock-delay", "picture-uri"],
    );
    fire(&Settings::new(SCHEMA_LOCKDOWN), &["disable-lock-screen"]);
}
