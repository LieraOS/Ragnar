//! Forwarding and link verbs — each validates its arguments, then
//! execs exactly one whitelisted tool call with an arg vector.

use super::exec::{reject, run, VerbResult};
use super::validate;

/// `ufw <args>` — the action itself is whitelisted; everything after
/// it is passed through to ufw unchanged, exactly like the shell
/// helper. `--force` is re-dispatched and limited to enable|delete|
/// reset so `ufw --force reset` stays possible but no destructive
/// combination sneaks past.
pub fn ufw(args: &[String]) -> VerbResult {
    let Some(action) = args.first().map(String::as_str) else {
        return reject("ufw: no action");
    };
    const ACTIONS: &[&str] = &[
        "status", "enable", "disable", "default", "allow", "deny", "reject", "limit", "delete",
        "logging", "reload", "reset", "insert",
    ];
    if action == "--force" {
        let Some(next) = args.get(1).map(String::as_str) else {
            return reject("ufw --force: missing action");
        };
        match next {
            "enable" | "delete" | "reset" => {}
            other => return reject(&format!("ufw --force not allowed for '{other}'")),
        }
    } else if !ACTIONS.contains(&action) {
        return reject(&format!("unsupported ufw action '{action}'"));
    }
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    run("ufw", &argv)
}

/// `ip [-4|-6] route add|del|replace <args>` — only route mutations;
/// never `ip link`, `ip addr`, `ip route flush`, `ip rule`.
pub fn ip(args: &[String]) -> VerbResult {
    let mut argv: Vec<&str> = Vec::new();
    let mut rest = args;
    if matches!(rest.first().map(String::as_str), Some("-4" | "-6")) {
        argv.push(rest[0].as_str());
        rest = &rest[1..];
    }
    if rest.first().map(String::as_str) != Some("route") {
        return reject(&format!(
            "ip {} not allowed",
            rest.first().map(String::as_str).unwrap_or("")
        ));
    }
    match rest.get(1).map(String::as_str) {
        Some("add" | "del" | "replace") => {}
        other => {
            return reject(&format!(
                "ip route {} not allowed",
                other.unwrap_or("")
            ))
        }
    }
    argv.push("route");
    argv.extend(rest[1..].iter().map(String::as_str));
    run("ip", &argv)
}

/// `resolvectl <verb> <args>` — the verb chain Heimdall uses; no
/// set-log-level, statistics, or arbitrary option injection.
pub fn resolvectl(args: &[String]) -> VerbResult {
    let Some(action) = args.first().map(String::as_str) else {
        return reject("resolvectl: no action");
    };
    const VERBS: &[&str] = &[
        "dns",
        "domain",
        "dnssec",
        "dnsovertls",
        "llmnr",
        "mdns",
        "flush-caches",
        "revert",
        "status",
    ];
    if !VERBS.contains(&action) {
        return reject(&format!("resolvectl {action} not allowed"));
    }
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    run("resolvectl", &argv)
}

/// `link <iface> up|down` — administrative link state; the switch
/// in the UI mirrors IFF_UP, and this is how it is set.
pub fn link(args: &[String]) -> VerbResult {
    let (Some(iface), Some(state)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("link: missing arguments");
    };
    if !validate::iface(iface) {
        return reject("bad iface");
    }
    match state {
        "up" | "down" => run("ip", &["link", "set", "dev", iface, state]),
        "mtu" => {
            let Some(mtu) = args.get(2).map(String::as_str) else {
                return reject("link mtu: missing value");
            };
            if !validate::digits(mtu) {
                return reject("bad mtu");
            }
            run("ip", &["link", "set", "dev", iface, "mtu", mtu])
        }
        other => reject(&format!("link {other} not allowed")),
    }
}

/// `linkadd <name> bridge` | `<name> bond <mode>` |
/// `<name> vlan <id> <parent>` — every field whitelisted, no
/// free-form `ip` arguments ever reach the exec.
pub fn linkadd(args: &[String]) -> VerbResult {
    let (Some(name), Some(kind)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("linkadd: missing arguments");
    };
    if !validate::iface(name) {
        return reject("bad name");
    }
    const BOND_MODES: &[&str] = &[
        "balance-rr",
        "active-backup",
        "balance-xor",
        "broadcast",
        "802.3ad",
        "balance-tlb",
        "balance-alb",
    ];
    match kind {
        "bridge" => run("ip", &["link", "add", "name", name, "type", "bridge"]),
        "bond" => {
            let mode = args.get(2).map(String::as_str).unwrap_or("");
            if !BOND_MODES.contains(&mode) {
                return reject("bad bond mode");
            }
            run(
                "ip",
                &["link", "add", "name", name, "type", "bond", "mode", mode],
            )
        }
        "vlan" => {
            let (Some(id), Some(parent)) = (
                args.get(2).map(String::as_str),
                args.get(3).map(String::as_str),
            ) else {
                return reject("linkadd vlan: missing id or parent");
            };
            let vid_ok = validate::digits(id)
                && id.parse::<u32>().map(|v| (1..=4094).contains(&v)).unwrap_or(false);
            if !vid_ok {
                return reject("vlan id out of range");
            }
            if !validate::iface(parent) {
                return reject("bad parent");
            }
            run(
                "ip",
                &["link", "add", "name", name, "type", "vlan", "id", id, "dev", parent],
            )
        }
        other => reject(&format!("linkadd {other} not allowed")),
    }
}

/// `linkdel <name>` — kernel refuses to delete physical NICs.
pub fn linkdel(args: &[String]) -> VerbResult {
    let Some(name) = args.first().map(String::as_str) else {
        return reject("linkdel: missing name");
    };
    if !validate::iface(name) {
        return reject("bad name");
    }
    run("ip", &["link", "del", name])
}

/// `linkmaster <member> <master|nomaster>` — enslave a link under
/// a bridge/bond or release it again.
pub fn linkmaster(args: &[String]) -> VerbResult {
    let (Some(member), Some(master)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("linkmaster: missing arguments");
    };
    if !validate::iface(member) || !validate::iface(master) {
        return reject("bad member or master");
    }
    if master == "nomaster" {
        run("ip", &["link", "set", "dev", member, "nomaster"])
    } else {
        run("ip", &["link", "set", "dev", member, "master", master])
    }
}

/// `rfkill wifi block|unblock` — soft block only; hardware switches
/// stay physical by definition.
pub fn rfkill(args: &[String]) -> VerbResult {
    let (Some(what), Some(how)) = (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) else {
        return reject("rfkill: missing arguments");
    };
    if what != "wifi" {
        return reject(&format!("rfkill {what} not allowed"));
    }
    match how {
        "block" | "unblock" => run("rfkill", &[how, "wifi"]),
        other => reject(&format!("rfkill {other} not allowed")),
    }
}
