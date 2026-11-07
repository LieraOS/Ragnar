// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — capability detection
//
// Two layers decide whether an action can even run:
//   1. logind Can* methods (CanSuspend, CanHibernate, …) — answers
//      "yes"/"na"/"challenge"; only an explicit "no" hides the tile.
//   2. Helper binaries (ragnar-session-quit) must exist in PATH.
// The result is combined with the user's show-* gsettings keys by
// the action table in actions.rs.

use gtk4::{gio, glib};
use std::path::PathBuf;

/// Query one org.freedesktop.login1.Manager Can* method.
/// Unknown/error answers are treated as "available" — hiding a
/// working action is worse than offering a failing one.
fn logind_can(method: &str) -> bool {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>) else {
        return true;
    };
    let call: Option<glib::Variant> = conn
        .call_sync(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            method,
            None::<&glib::Variant>,
            None,
            gio::DBusCallFlags::NONE,
            2000,
            None::<&gio::Cancellable>,
        )
        .ok();
    match call.and_then(|v| v.child_value(0).str().map(|s| s.to_string())) {
        Some(answer) => answer != "no",
        None => true,
    }
}

/// Resolve a binary name against PATH — `which` without a subprocess.
fn binary_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<PathBuf>>())
        .unwrap_or_default()
        .iter()
        .any(|dir| dir.join(name).is_file())
}

/// Hardware/session capability per action id.
pub fn capable(action_id: &str) -> bool {
    match action_id {
        "suspend" => logind_can("CanSuspend"),
        "hibernate" => logind_can("CanHibernate"),
        "reboot" => logind_can("CanReboot") && binary_exists("ragnar-session-quit"),
        "shutdown" => logind_can("CanPowerOff") && binary_exists("ragnar-session-quit"),
        "logout" => binary_exists("ragnar-session-quit"),
        // Screen lock goes through the session's Screenlock D-Bus
        // service; offer it whenever the bus name can be activated.
        _ => true,
    }
}
