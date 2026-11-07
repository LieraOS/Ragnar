// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-shared — module identifiers for the settings GUI
// Copyright (C) 2026 LieraOS Project

use serde::{Deserialize, Serialize};

/// Available modules in the settings GUI. Each module is a page
/// the user can navigate to from the sidebar. New modules are
/// added here as they are implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Module {
    Display,
    Audio,
    Network,
    Input,
    DateTime,
    Power,
    Appearance,
    Accounts,
    Privacy,
    Storage,
    DefaultApps,
    Updates,
    About,
}

impl Module {
    /// Human-readable label shown in the sidebar (English).
    pub fn label(&self) -> &'static str {
        match self {
            Module::Display => "Display",
            Module::Audio => "Audio",
            Module::Network => "Network",
            Module::Input => "Input",
            Module::DateTime => "Date & Language",
            Module::Power => "Power",
            Module::Appearance => "Appearance",
            Module::Accounts => "Accounts",
            Module::Privacy => "Privacy",
            Module::Storage => "Storage",
            Module::DefaultApps => "Default Apps",
            Module::Updates => "Updates",
            Module::About => "About",
        }
    }

    /// Icon name (GTK symbolic icon) for the sidebar entry.
    pub fn icon(&self) -> &'static str {
        match self {
            Module::Display => "video-display-symbolic",
            Module::Audio => "audio-volume-high-symbolic",
            Module::Network => "network-wired-symbolic",
            Module::Input => "input-keyboard-symbolic",
            Module::DateTime => "preferences-system-time-symbolic",
            Module::Power => "battery-symbolic",
            Module::Appearance => "applications-graphics-symbolic",
            Module::Accounts => "avatar-default-symbolic",
            Module::Privacy => "preferences-system-privacy-symbolic",
            Module::Storage => "drive-harddisk-symbolic",
            Module::DefaultApps => "emblem-default-symbolic",
            Module::Updates => "software-update-available-symbolic",
            Module::About => "help-about-symbolic",
        }
    }

    /// All modules in sidebar order.
    pub fn all() -> &'static [Module] {
        &[
            Module::Display,
            Module::Audio,
            Module::Network,
            Module::Input,
            Module::DateTime,
            Module::Power,
            Module::Appearance,
            Module::Accounts,
            Module::Privacy,
            Module::Storage,
            Module::DefaultApps,
            Module::Updates,
            Module::About,
        ]
    }
}
