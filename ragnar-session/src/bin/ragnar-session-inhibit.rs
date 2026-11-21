//! ragnar-session-inhibit — run a command while inhibiting session
//! actions (logout, suspend, idle, user switch) via the running
//! `org.gnome.SessionManager`.
//!
//! Rust port of the C `gnome-session-inhibit`-equivalent helper from
//! the old gnome-session fork package.

use std::os::unix::process::ExitStatusExt;
use zbus::Connection;

/// GNOME inhibitor flag bits (org.gnome.SessionManager contract).
const FLAG_LOGOUT: u32 = 1;
const FLAG_SWITCH_USER: u32 = 2;
const FLAG_SUSPEND: u32 = 4;
const FLAG_IDLE: u32 = 8;

fn usage() -> ! {
    eprintln!("Usage: ragnar-session-inhibit [OPTIONS...] [COMMAND [ARG...]]");
    eprintln!("  --inhibit ACTION[:ACTION...]  actions: logout, switch-user,");
    eprintln!("                               suspend, shutdown, idle");
    eprintln!("  --app-id ID                  application identifier");
    eprintln!("  --reason TEXT                reason shown in dialogs");
    eprintln!("  --inhibit-only               hold the lock until killed");
    eprintln!("  --list                       list active inhibitors");
    std::process::exit(1);
}

/// Maps an action name to its inhibitor flag bit.
fn action_flag(name: &str) -> Option<u32> {
    match name {
        "logout" => Some(FLAG_LOGOUT),
        "switch-user" => Some(FLAG_SWITCH_USER),
        "suspend" | "shutdown" => Some(FLAG_SUSPEND),
        "idle" => Some(FLAG_IDLE),
        _ => None,
    }
}

#[tokio::main]
async fn main() {
    // Default flags mirror gnome-session-inhibit: everything except idle.
    let mut flags = FLAG_LOGOUT | FLAG_SWITCH_USER | FLAG_SUSPEND;
    let mut app_id = "unknown".to_string();
    let mut reason = "not specified".to_string();
    let mut inhibit_only = false;
    let mut list_only = false;
    let mut command: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--inhibit" => {
                let spec = args.next().unwrap_or_else(|| usage());
                flags = 0;
                for name in spec.split(':') {
                    match action_flag(name) {
                        Some(bit) => flags |= bit,
                        None => {
                            eprintln!("ragnar-session-inhibit: unknown action '{name}'");
                            std::process::exit(1);
                        }
                    }
                }
            }
            "--app-id" => app_id = args.next().unwrap_or_else(|| usage()),
            "--reason" => reason = args.next().unwrap_or_else(|| usage()),
            "--inhibit-only" => inhibit_only = true,
            "--list" => list_only = true,
            "--version" => {
                println!("ragnar-session-inhibit {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ if arg.starts_with("--") => usage(),
            _ => {
                command.push(arg);
                command.extend(args);
                break;
            }
        }
    }

    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ragnar-session-inhibit: couldn't connect to session bus: {e}");
            std::process::exit(1);
        }
    };
    let dest = Some("org.gnome.SessionManager");
    let path = "/org/gnome/SessionManager";
    let iface = Some("org.gnome.SessionManager");

    if list_only {
        // GetInhibitors returns object paths; our manager keeps the
        // registry internally, so enumerate what it reports.
        let paths: Vec<zbus::zvariant::OwnedObjectPath> = conn
            .call_method(dest, path, iface, "GetInhibitors", &())
            .await
            .map(|m| m.body().deserialize().unwrap_or_default())
            .unwrap_or_default();
        if paths.is_empty() {
            println!("No inhibitors");
        } else {
            for p in paths {
                println!("{}", p.as_str());
            }
        }
        return;
    }

    // toplevel_xid is X11-specific — 0 on Wayland.
    let cookie: u32 = match conn
        .call_method(dest, path, iface, "Inhibit", &(&app_id, 0u32, &reason, flags))
        .await
    {
        Ok(m) => match m.body().deserialize() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ragnar-session-inhibit: bad reply: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("ragnar-session-inhibit: Inhibit failed: {e}");
            std::process::exit(1);
        }
    };

    // Hold the lock while the command runs (or until killed with
    // --inhibit-only), then release it again.
    let status = if inhibit_only {
        let _ = tokio::signal::ctrl_c().await;
        std::process::ExitStatus::from_raw(0)
    } else if command.is_empty() {
        // No command: keep holding — same as --inhibit-only.
        let _ = tokio::signal::ctrl_c().await;
        std::process::ExitStatus::from_raw(0)
    } else {
        let mut cmd = tokio::process::Command::new(&command[0]);
        cmd.args(&command[1..]);
        match cmd.status().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ragnar-session-inhibit: unable to start: {e}");
                std::process::exit(1);
            }
        }
    };

    let _ = conn
        .call_method(dest, path, iface, "Uninhibit", &(cookie))
        .await;
    std::process::exit(status.code().unwrap_or(1));
}
