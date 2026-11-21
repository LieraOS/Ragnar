//! ragnar-session-quit — request logout/reboot/shutdown through the
//! running `org.gnome.SessionManager`.
//!
//! Rust port of the C helper from the old gnome-session fork package.
//! Bound to session keybindings (labwc rc.xml calls this tool); it
//! forwards the request to ragnar-session, which runs the
//! EndSessionDialog flow (ragnar-power-dialog) unless confirmation
//! was disabled.

use zbus::Connection;

/// gnome-session logout modes.
const MODE_NORMAL: u32 = 0;
const MODE_NO_CONFIRM: u32 = 1;
const MODE_FORCE: u32 = 2;

fn usage() -> ! {
    eprintln!("Usage: ragnar-session-quit [--logout] [--power-off] [--reboot]");
    eprintln!("       [--force] [--no-prompt]");
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    // Parse CLI flags — mutually exclusive actions, last one wins like
    // the original tool; modifiers select the logout mode.
    let mut action = "logout";
    let mut mode = MODE_NORMAL;
    let mut have_action = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--logout" => {
                action = "logout";
                have_action = true;
            }
            "--power-off" => {
                action = "power-off";
                have_action = true;
            }
            "--reboot" => {
                action = "reboot";
                have_action = true;
            }
            "--force" => mode = MODE_FORCE,
            "--no-prompt" => mode = MODE_NO_CONFIRM,
            _ => usage(),
        }
    }
    let _ = have_action; // default action is logout without flags

    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ragnar-session-quit: couldn't connect to session bus: {e}");
            std::process::exit(1);
        }
    };

    // Translate the action into the matching SessionManager call.
    let result: zbus::Result<()> = match action {
        "power-off" => {
            conn.call_method(
                Some("org.gnome.SessionManager"),
                "/org/gnome/SessionManager",
                Some("org.gnome.SessionManager"),
                "Shutdown",
                &(),
            )
            .await
            .map(|_| ())
        }
        "reboot" => {
            conn.call_method(
                Some("org.gnome.SessionManager"),
                "/org/gnome/SessionManager",
                Some("org.gnome.SessionManager"),
                "Reboot",
                &(),
            )
            .await
            .map(|_| ())
        }
        _ => {
            conn.call_method(
                Some("org.gnome.SessionManager"),
                "/org/gnome/SessionManager",
                Some("org.gnome.SessionManager"),
                "Logout",
                &(mode),
            )
            .await
            .map(|_| ())
        }
    };

    if let Err(e) = result {
        eprintln!("ragnar-session-quit: failed to call {action}: {e}");
        std::process::exit(1);
    }
}
