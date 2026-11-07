// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — session actions
//
// One static table describes every action: stable id (used for the
// gsettings show-<id> key), icon, label, destructive flag and the
// real command. Commands are identical to the ones the session
// itself uses (taskbar menu strip, labwc rc.xml):
//   lock     → Screenlock D-Bus (gtklock underneath)
//   logout   → ragnar-session-quit --logout
//   suspend  → systemctl suspend          (logind)
//   hibernate→ systemctl hibernate        (logind)
//   reboot   → ragnar-session-quit --reboot
//   shutdown → ragnar-session-quit --power-off

use crate::{caps, config, i18n::tr};
use gtk4::gio;
use std::process::Command;

/// One user-selectable session action.
pub struct Action {
    /// Stable id — also the suffix of the `show-<id>` gsettings key.
    pub id: &'static str,
    /// Icon name (Adwaita symbolic).
    pub icon: &'static str,
    /// German msgid — translated at build time via `tr()`.
    pub label: &'static str,
    /// Shutdown/restart get a warning tint (CSS class
    /// `power-action-destructive`).
    pub destructive: bool,
}

/// Every action the dialog can offer, in display order.
pub const ACTIONS: &[Action] = &[
    Action {
        id: "lock",
        icon: "system-lock-screen-symbolic",
        label: "Bildschirm sperren",
        destructive: false,
    },
    Action {
        id: "logout",
        icon: "system-log-out-symbolic",
        label: "Abmelden",
        destructive: false,
    },
    Action {
        id: "suspend",
        icon: "system-suspend-symbolic",
        label: "Bereitschaft",
        destructive: false,
    },
    Action {
        id: "hibernate",
        icon: "system-hibernate-symbolic",
        label: "Ruhezustand",
        destructive: false,
    },
    Action {
        id: "reboot",
        icon: "system-reboot-symbolic",
        label: "Neustart",
        destructive: true,
    },
    Action {
        id: "shutdown",
        icon: "system-shutdown-symbolic",
        label: "Herunterfahren",
        destructive: true,
    },
];

/// Actions that survive both filters: the user enabled them in
/// Ragnar Settings AND the machine can actually perform them.
pub fn available() -> Vec<&'static Action> {
    ACTIONS
        .iter()
        .filter(|a| config::visible(a.id) && caps::capable(a.id))
        .collect()
}

/// Execute an action — fire and forget; the window closes
/// immediately afterwards (lock takes the screen, reboot kills us
/// anyway).
pub fn run(id: &str) {
    // --no-prompt on logout: this dialog IS the confirmation —
    // session-quit would otherwise fire its own (dead) prompt path.
    // Reboot/shutdown go straight to logind: session-quit ignores
    // flags for them and routes through the old endsession flow,
    // which swallows the first request.
    match id {
        "lock" => lock_screen(),
        "logout" => spawn("ragnar-session-quit", &["--logout", "--no-prompt"]),
        "suspend" => spawn("systemctl", &["suspend"]),
        "hibernate" => spawn("systemctl", &["hibernate"]),
        "reboot" => spawn("systemctl", &["reboot"]),
        "shutdown" => spawn("systemctl", &["poweroff"]),
        _ => {}
    }
}

/// Ask the session's Screenlock service to lock — the same D-Bus
/// call the taskbar strip and swayidle use (gtklock underneath).
fn lock_screen() {
    if let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        let _ = conn.call_sync(
            Some("org.lieraos.ragnar.Screenlock"),
            "/org/lieraos/ragnar/Screenlock",
            "org.lieraos.ragnar.Screenlock",
            "Lock",
            None,
            None,
            gio::DBusCallFlags::NONE,
            2000,
            None::<&gio::Cancellable>,
        );
    }
}

/// Spawn a command detached. Failures are non-fatal — nothing to
/// show the user in a window that is about to disappear anyway.
fn spawn(program: &str, args: &[&str]) {
    let _ = Command::new(program).args(args).spawn();
}

/// Translated label for an action — used by the tile builder.
pub fn label(a: &Action) -> String {
    tr(a.label)
}
