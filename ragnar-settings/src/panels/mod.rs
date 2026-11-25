// Copyright (C) Jörg Schröder
//! Embedded settings panels.
//!
//! A panel is a GTK widget built lazily when its tile is activated.
//! Panels are identified by their `id` from the registry catalog.

pub mod about;
pub mod accessibility;
pub mod appearance;
pub mod autostart;
pub mod cloud;
pub mod common;
pub mod compositor;
pub mod datetime;
pub mod default_apps;
pub mod desktop;
pub mod display;
pub mod ini;
pub mod input;
pub mod language;
pub mod media;
pub mod nightlight;
pub mod notifications;
pub mod osd;
pub mod placeholder;
pub mod power;
pub mod printers;
pub mod privacy;
pub mod rcxml;
pub mod shortcuts;
pub mod sounds;
pub mod storage;
pub mod updates;
pub mod users;
pub mod windowrules;

use gtk4::Widget;

/// Build the widget for the panel `id`.
///
/// Returns `None` when the id is unknown or the panel's backend is
/// unavailable; the window then shows the placeholder page.
pub fn build(id: &str) -> Option<Widget> {
    match id {
        "about" => Some(about::build()),
        "notifications" => Some(notifications::build()),
        "appearance" => Some(appearance::build()),
        "cloud" => Some(cloud::build()),
        "compositor" => Some(compositor::build()),
        "windowrules" => Some(windowrules::build()),
        "autostart" => Some(autostart::build()),
        "datetime" => Some(datetime::build()),
        "display" => Some(display::build()),
        "default-apps" => Some(default_apps::build()),
        "desktop" => Some(desktop::build()),
        "input" => Some(input::build()),
        "language" => Some(language::build()),
        "media" => Some(media::build()),
        "nightlight" => Some(nightlight::build()),
        "updates" => Some(updates::build()),
        "accessibility" => Some(accessibility::build()),
        "power" => Some(power::build()),
        "privacy" => Some(privacy::build()),
        "shortcuts" => Some(shortcuts::build()),
        "printers" => Some(printers::build()),
        "sounds" => Some(sounds::build()),
        "storage" => Some(storage::build()),
        "users" => Some(users::build()),
        other => placeholder::build(other),
    }
}
