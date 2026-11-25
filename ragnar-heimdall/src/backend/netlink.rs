// Copyright (C) Jörg Schröder
//! Link-level and addressing control: link up/down and the
//! DHCP↔static IPv4 switch, written as a networkd drop-in so the
//! choice survives reboots. All mutations go through `pkexec` +
//! `ragnar-heimdall-helper` with strict validation on both sides.

use anyhow::{Context, Result};
use std::process::Command;

/// Bring a kernel interface up or down (`ip link set dev <if>`).
pub fn set_link(iface: &str, up: bool) -> Result<()> {
    run(&["link", iface, if up { "up" } else { "down" }])
        .with_context(|| format!("link {} {}", iface, if up { "up" } else { "down" }))
}

/// Switch a link to DHCP addressing — writes
/// `/etc/systemd/network/60-ragnar-heimdall-<if>.network` with `DHCP=yes`
/// and reloads networkd.
pub fn set_dhcp(iface: &str) -> Result<()> {
    run(&["networkd", iface, "dhcp"]).with_context(|| format!("DHCP on {iface}"))
}

/// Switch a link to a static IPv4: `cidr` like `192.168.1.10/24`,
/// `gw` a plain gateway address. Validated here and again in the
/// helper — a malformed value must never reach the config file.
pub fn set_static(iface: &str, cidr: &str, gw: &str) -> Result<()> {
    anyhow::ensure!(valid_cidr(cidr), "invalid address/mask: {cidr}");
    anyhow::ensure!(valid_ipv4(gw), "invalid gateway: {gw}");
    run(&["networkd", iface, "static", cidr, gw]).with_context(|| format!("static IPv4 on {iface}"))
}

/// Change the interface MTU (`ip link set dev <if> mtu <n>`).
pub fn set_mtu(iface: &str, mtu: u32) -> Result<()> {
    anyhow::ensure!((68..=9000).contains(&mtu), "invalid MTU: {mtu}");
    run(&["link", iface, "mtu", &mtu.to_string()]).with_context(|| format!("MTU {mtu} on {iface}"))
}

/// Per-link DNS servers via `resolvectl dns <if> <servers…>`; an
/// empty list reverts to DHCP-provided servers (`revert`).
pub fn set_dns(iface: &str, servers: &str) -> Result<()> {
    let trimmed = servers.trim();
    if trimmed.is_empty() {
        return run(&["resolvectl", "revert", iface])
            .with_context(|| format!("revert DNS on {iface}"));
    }
    for s in trimmed.split([',', ' ']).filter(|s| !s.is_empty()) {
        anyhow::ensure!(
            s.parse::<std::net::IpAddr>().is_ok(),
            "invalid DNS server: {s}"
        );
    }
    let mut args = vec!["resolvectl", "dns", iface];
    args.extend(trimmed.split([',', ' ']).filter(|s| !s.is_empty()));
    run(&args).with_context(|| format!("DNS on {iface}"))
}

/// Kernel whitelist of bond modes the helper accepts — mirrored in
/// the helper's `case` so both sides validate identically.
pub const BOND_MODES: &[&str] = &[
    "balance-rr",
    "active-backup",
    "balance-xor",
    "broadcast",
    "802.3ad",
    "balance-tlb",
    "balance-alb",
];

/// Create a virtual interface through the helper's strict `linkadd`
/// verb: `vlan` needs id+parent, `bond` needs a whitelisted mode,
/// `bridge` stands alone.
pub fn add_virtual(
    name: &str,
    kind: &str,
    parent: &str,
    vlan_id: u32,
    bond_mode: &str,
) -> Result<()> {
    anyhow::ensure!(valid_name(name), "bad interface name: {name}");
    match kind {
        "vlan" => {
            anyhow::ensure!((1..=4094).contains(&vlan_id), "bad VLAN id: {vlan_id}");
            anyhow::ensure!(valid_name(parent), "bad parent interface: {parent}");
            run(&["linkadd", name, "vlan", &vlan_id.to_string(), parent])
        }
        "bridge" => run(&["linkadd", name, "bridge"]),
        "bond" => {
            anyhow::ensure!(
                BOND_MODES.contains(&bond_mode),
                "bad bond mode: {bond_mode}"
            );
            run(&["linkadd", name, "bond", bond_mode])
        }
        _ => anyhow::bail!("unsupported virtual type: {kind}"),
    }
    .with_context(|| format!("create {kind} {name}"))
}

/// Interface names the helper accepts: kernel rules minus the few
/// characters that could confuse the shell or sysfs paths.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
}

/// Delete a virtual interface (`ip link del <name>`). The kernel
/// refuses to remove physical NICs itself — the helper just relays.
pub fn del_virtual(name: &str) -> Result<()> {
    anyhow::ensure!(valid_name(name), "bad interface name: {name}");
    run(&["linkdel", name]).with_context(|| format!("delete {name}"))
}

/// Per-interface IPv6 kernel toggle: sysctl net.ipv6.conf.<if>.disable_ipv6.
/// Note the inversion — the sysctl *disables*, so `enabled` maps to `0`.
pub fn set_ipv6(iface: &str, enabled: bool) -> Result<()> {
    run(&["sysctl", iface, if enabled { "0" } else { "1" }])
        .with_context(|| format!("IPv6 {} on {iface}", if enabled { "on" } else { "off" }))
}

/// Route preference for this link: `Some(metric)` writes a networkd
/// drop-in so DHCP routes from this interface win (lower metric) or
/// lose (higher metric) against other links; `None` removes the
/// override and returns to automatic selection.
pub fn set_route_metric(iface: &str, metric: Option<u32>) -> Result<()> {
    let arg;
    if let Some(m) = metric {
        anyhow::ensure!((1..=4294967295).contains(&m), "invalid metric: {m}");
        arg = m.to_string();
    } else {
        arg = "auto".to_string();
    }
    run(&["metric", iface, &arg]).with_context(|| format!("route metric on {iface}"))
}

/// Enslave `member` under a bridge/bond `master`, or release it with
/// `None` (`ip link set dev <member> master/nomaster`).
pub fn set_master(member: &str, master: Option<&str>) -> Result<()> {
    match master {
        Some(m) => {
            run(&["linkmaster", member, m]).with_context(|| format!("enslave {member} to {m}"))
        }
        None => {
            run(&["linkmaster", member, "nomaster"]).with_context(|| format!("release {member}"))
        }
    }
}

/// `A.B.C.D/P` with octets 0-255 and prefix 0-32.
fn valid_cidr(cidr: &str) -> bool {
    let Some((ip, prefix)) = cidr.split_once('/') else {
        return false;
    };
    valid_ipv4(ip) && prefix.parse::<u8>().map(|p| p <= 32).unwrap_or(false)
}

/// Strict dotted quad, each octet ≤ 255.
fn valid_ipv4(ip: &str) -> bool {
    let mut parts = ip.split('.');
    (0..4).all(|_| parts.next().and_then(|p| p.parse::<u8>().ok()).is_some())
        && parts.next().is_none()
}

/// Execute `<verb> <args>` through pkexec + helper.
fn run(args: &[&str]) -> Result<()> {
    let prog = super::ufw::HELPER_PATH;
    anyhow::ensure!(
        std::path::Path::new(prog).is_file(),
        "ragnar-heimdall-helper not installed"
    );
    let output = Command::new("pkexec")
        .arg(prog)
        .args(args)
        .output()
        .context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("helper failed ({}): {}", output.status, stderr)
}

#[cfg(test)]
mod tests {
    #[test]
    fn cidr_validation() {
        assert!(super::valid_cidr("192.168.1.10/24"));
        assert!(!super::valid_cidr("192.168.1.10"));
        assert!(!super::valid_cidr("999.0.0.1/24"));
        assert!(!super::valid_cidr("1.2.3.4/33"));
        assert!(super::valid_ipv4("10.0.0.1"));
        assert!(!super::valid_ipv4("10.0.0.1;rm"));
    }
}
