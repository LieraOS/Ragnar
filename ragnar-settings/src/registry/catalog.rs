//! The concrete settings catalog.
//!
//! Category order defines display order on the hub. Entries reference
//! category ids; each entry names its embedded panel id or the external
//! program to launch.

use super::{Action, Entry};
use crate::backend::Requirement;

/// One category shown as a group header on the hub.
#[derive(Debug, Clone, Copy)]
pub struct Category {
    /// Stable machine id matched by `Entry::category`.
    pub id: &'static str,
    /// English source string, translated at display time.
    pub title: &'static str,
}

/// Category groups in display order.
pub fn categories() -> &'static [Category] {
    &[
        Category {
            id: "system",
            title: "System",
        },
        Category {
            id: "personalization",
            title: "Personalization",
        },
        Category {
            id: "network",
            title: "Network and Internet",
        },
        Category {
            id: "programs",
            title: "Programs",
        },
        Category {
            id: "devices",
            title: "Devices",
        },
        Category {
            id: "accounts",
            title: "Accounts and Security",
        },
    ]
}

/// All catalog entries. Order inside a category defines tile order.
fn catalog() -> &'static [Entry] {
    &[
        // --- system -------------------------------------------------
        Entry {
            id: "about",
            title: "About this system",
            subtitle: "Hardware, operating system and Ragnar version",
            icon: "computer-symbolic",
            category: "system",
            action: Action::Panel("about"),
            requirement: Requirement::Always,
            keywords: &["info", "hardware", "version", "os"],
        },
        Entry {
            id: "datetime",
            title: "Date & Time",
            subtitle: "Timezone, NTP and clock settings",
            icon: "preferences-system-time-symbolic",
            category: "system",
            action: Action::Panel("datetime"),
            requirement: Requirement::Always,
            keywords: &["time", "date", "timezone", "ntp", "clock"],
        },
        Entry {
            id: "language",
            title: "Region & Language",
            subtitle: "System language and formats",
            icon: "preferences-desktop-locale-symbolic",
            category: "system",
            action: Action::Panel("language"),
            requirement: Requirement::Always,
            keywords: &["language", "locale", "region", "german", "english"],
        },
        Entry {
            id: "accessibility",
            title: "Accessibility",
            subtitle: "Text size and contrast options",
            icon: "preferences-desktop-accessibility-symbolic",
            category: "system",
            action: Action::Panel("accessibility"),
            requirement: Requirement::Always,
            keywords: &["accessibility", "font size", "contrast"],
        },
        Entry {
            id: "sound",
            title: "Sound",
            subtitle: "Volume, devices and effects",
            icon: "audio-speakers-symbolic",
            category: "system",
            action: Action::Launch("lieraaudio"),
            requirement: Requirement::Program("lieraaudio"),
            keywords: &["audio", "volume", "microphone", "lieraaudio"],
        },
        Entry {
            id: "display",
            title: "Displays",
            subtitle: "Resolution, arrangement and scaling",
            icon: "video-display-symbolic",
            category: "system",
            action: Action::Panel("display"),
            requirement: Requirement::Always,
            keywords: &["monitor", "resolution", "screen"],
        },
        Entry {
            id: "nightlight",
            title: "Night light",
            subtitle: "Warmer colors at night",
            icon: "night-light-symbolic",
            category: "system",
            action: Action::Panel("nightlight"),
            requirement: Requirement::Always,
            keywords: &["night", "blue light", "gamma", "color temperature"],
        },
        // --- personalization ---------------------------------------
        Entry {
            id: "appearance",
            title: "Appearance",
            subtitle: "Wallpaper, themes and icons",
            icon: "preferences-desktop-appearance-symbolic",
            category: "personalization",
            action: Action::Panel("appearance"),
            requirement: Requirement::Always,
            keywords: &["background", "wallpaper", "theme", "icons"],
        },
        Entry {
            id: "desktop",
            title: "Panel and desktop",
            subtitle: "Panel layout, applets and autostart",
            icon: "preferences-system-symbolic",
            category: "personalization",
            action: Action::Panel("desktop"),
            requirement: Requirement::Schema("org.lieraos.ragnar.panel"),
            keywords: &["panel", "applet", "autostart", "taskbar"],
        },
        Entry {
            id: "notifications",
            title: "Notifications",
            subtitle: "Notification display, duration and monitor",
            icon: "preferences-system-notifications-symbolic",
            category: "personalization",
            action: Action::Panel("notifications"),
            requirement: Requirement::Always,
            keywords: &["notification", "raven", "alert", "popup"],
        },
        Entry {
            id: "taskbar",
            title: "Taskbar",
            subtitle: "Panel layout, applets and workspaces",
            icon: "goa-panel-symbolic",
            category: "personalization",
            action: Action::Launch("ragnar-taskbar --settings"),
            requirement: Requirement::Program("ragnar-taskbar"),
            keywords: &["taskbar", "panel", "bar", "applet", "dock"],
        },
        Entry {
            id: "sounds",
            title: "System sounds",
            subtitle: "Sound themes for system events",
            icon: "audio-volume-high-symbolic",
            category: "personalization",
            action: Action::Panel("sounds"),
            requirement: Requirement::Always,
            keywords: &["theme", "bell", "notification sound"],
        },
        // --- network ------------------------------------------------
        Entry {
            id: "network",
            title: "Network and Bluetooth",
            subtitle: "Connections, Wi-Fi and Bluetooth devices",
            icon: "network-wired-symbolic",
            category: "network",
            action: Action::Launch("ragnar-heimdall"),
            requirement: Requirement::Program("ragnar-heimdall"),
            keywords: &["wifi", "ethernet", "bluetooth", "ragnar-heimdall"],
        },
        // --- programs ----------------------------------------------
        Entry {
            id: "software",
            title: "Software",
            subtitle: "Coming soon — managed by a dedicated app",
            icon: "system-software-install-symbolic",
            category: "programs",
            action: Action::Panel("software"),
            requirement: Requirement::Always,
            keywords: &["install", "remove", "apps", "packages"],
        },
        Entry {
            id: "updates",
            title: "System updates",
            subtitle: "Check for and install updates",
            icon: "software-update-available-symbolic",
            category: "programs",
            action: Action::Panel("updates"),
            requirement: Requirement::Always,
            keywords: &["update", "upgrade", "packages", "apt"],
        },
        Entry {
            id: "default-apps",
            title: "Default applications",
            subtitle: "Which app opens which file type",
            icon: "preferences-desktop-apps-symbolic",
            category: "programs",
            action: Action::Panel("default-apps"),
            requirement: Requirement::Always,
            keywords: &["mime", "default", "browser", "filetype"],
        },
        // --- devices ------------------------------------------------
        Entry {
            id: "shortcuts",
            title: "Keyboard shortcuts",
            subtitle: "Global key bindings",
            icon: "preferences-desktop-keyboard-shortcuts-symbolic",
            category: "devices",
            action: Action::Panel("shortcuts"),
            requirement: Requirement::Always,
            keywords: &["shortcut", "hotkey", "keybinding", "keys"],
        },
        Entry {
            id: "media",
            title: "Removable media",
            subtitle: "Automount and media handling",
            icon: "media-removable-symbolic",
            category: "devices",
            action: Action::Panel("media"),
            requirement: Requirement::Always,
            keywords: &["usb", "automount", "removable", "media"],
        },
        Entry {
            id: "input",
            title: "Mouse & Keyboard",
            subtitle: "Pointer speed, scrolling and layout",
            icon: "input-keyboard-symbolic",
            category: "devices",
            action: Action::Panel("input"),
            requirement: Requirement::Always,
            keywords: &["mouse", "keyboard", "touchpad", "scroll", "layout"],
        },
        Entry {
            id: "power",
            title: "Power",
            subtitle: "Power profile and battery status",
            icon: "power-profile-balanced-symbolic",
            category: "devices",
            action: Action::Panel("power"),
            requirement: Requirement::Always,
            keywords: &["power", "battery", "energy", "profile"],
        },
        Entry {
            id: "storage",
            title: "Storage",
            subtitle: "Disks and free space",
            icon: "drive-harddisk-symbolic",
            category: "devices",
            action: Action::Panel("storage"),
            requirement: Requirement::Always,
            keywords: &["disk", "storage", "space", "drive"],
        },
        Entry {
            id: "printers",
            title: "Printers",
            subtitle: "Add and manage printers",
            icon: "printer-symbolic",
            category: "devices",
            action: Action::Panel("printers"),
            requirement: Requirement::Always,
            keywords: &["print", "cups"],
        },
        // --- accounts ------------------------------------------------
        Entry {
            id: "cloud",
            title: "Cloud",
            subtitle: "Cloud accounts for calendar sync and WebDAV",
            icon: "folder-remote-symbolic",
            category: "accounts",
            action: Action::Panel("cloud"),
            requirement: Requirement::Always,
            keywords: &["cloud", "nextcloud", "owncloud", "webdav", "caldav", "sync"],
        },
        Entry {
            id: "privacy",
            title: "Privacy",
            subtitle: "Lock screen and location options",
            icon: "preferences-system-privacy-symbolic",
            category: "accounts",
            action: Action::Panel("privacy"),
            requirement: Requirement::Always,
            keywords: &["privacy", "lock", "location", "screen lock"],
        },
        Entry {
            id: "users",
            title: "User accounts",
            subtitle: "Users, passwords and permissions",
            icon: "system-users-symbolic",
            category: "accounts",
            action: Action::Panel("users"),
            requirement: Requirement::Always,
            keywords: &["password", "account", "admin"],
        },
    ]
}

/// Entries belonging to `category`, in catalog order.
pub fn entries_in(category: &str) -> Vec<&'static Entry> {
    catalog()
        .iter()
        .filter(|e| e.category == category)
        .collect()
}

/// All entries regardless of category (used by search).
pub fn all_entries() -> &'static [Entry] {
    catalog()
}
