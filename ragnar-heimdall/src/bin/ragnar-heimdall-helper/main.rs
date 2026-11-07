//! Privileged Heimdall helper — the Rust rewrite of the former
//! shell script, invoked via pkexec under
//! `org.lieraos.ragnar.Heimdall.admin` (auth_admin_keep).
//!
//! Design contract (unchanged):
//! - dispatch on argv[1] onto a closed verb set
//! - every argument validated BEFORE any command or file is touched
//! - children are spawned via `Command::new` arg vectors — no shell
//!   exists anywhere in this binary, so expansion/injection is
//!   structurally impossible
//! - exit code 0 = success, 2 = rejected input, child's own code
//!   is propagated for runtime failures

mod config;
mod exec;
mod iwd_profile;
mod validate;
mod verbs;
mod vpn;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!("ragnar-heimdall-helper: no verb given");
        return ExitCode::from(2);
    };
    let rest = &args[1..];
    let result = match verb {
        "ufw" => verbs::ufw(rest),
        "ip" => verbs::ip(rest),
        "resolvectl" => verbs::resolvectl(rest),
        "vpn" => vpn::run(rest),
        "iwd" => config::iwd(rest),
        "link" => verbs::link(rest),
        "linkadd" => verbs::linkadd(rest),
        "linkdel" => verbs::linkdel(rest),
        "linkmaster" => verbs::linkmaster(rest),
        "sysctl" => config::sysctl(rest),
        "metric" => config::metric(rest),
        "networkd" => config::networkd(rest),
        "rfkill" => verbs::rfkill(rest),
        "iwd-profile" => iwd_profile::run(rest),
        other => {
            eprintln!("ragnar-heimdall-helper: unsupported tool '{other}'");
            Err(())
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(()) => ExitCode::from(2),
    }
}
