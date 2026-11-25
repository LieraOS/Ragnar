// Copyright (C) Jörg Schröder
//! System-level network facts for the dashboard: default route,
//! configured DNS resolvers, and the machine's hostname.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

/// `ip -j route show default` record.
#[derive(Debug, Deserialize)]
struct IpRoute {
    #[serde(default)]
    gateway: Option<String>,
    #[serde(default)]
    dev: Option<String>,
}

/// First default route as `"<gateway> via <interface>"`, if any.
pub fn default_route() -> Result<Option<String>> {
    let output = Command::new("ip")
        .args(["-j", "route", "show", "default"])
        .output()
        .context("failed to run `ip route`")?;
    anyhow::ensure!(
        output.status.success(),
        "ip route exited with {}",
        output.status
    );
    let routes: Vec<IpRoute> =
        serde_json::from_slice(&output.stdout).context("failed to parse `ip -j route`")?;
    Ok(routes.first().map(|r| match (&r.gateway, &r.dev) {
        (Some(gw), Some(dev)) => format!("{gw} via {dev}"),
        (Some(gw), None) => gw.clone(),
        (None, Some(dev)) => format!("via {dev}"),
        (None, None) => String::new(),
    }))
}

/// Nameservers configured in `/etc/resolv.conf`, in file order.
pub fn dns_servers() -> Vec<String> {
    std::fs::read_to_string("/etc/resolv.conf")
        .map(|s| parse_resolv(&s))
        .unwrap_or_default()
}

/// Best-effort DHCP lease for `iface` through a well-known client.
/// Link setup needs root, so the client runs under `pkexec` — the
/// user confirms via the polkit dialog.
///
/// This fallback exists for systems where nothing else configures
/// the link. On the LieraOS target stack systemd-networkd already
/// hands out leases on `en*`/`wl*` (50-ragnar-heimdall.network), so running
/// a second DHCP client here would fight it and pop an unexpected
/// authentication dialog — in that case the call returns early.
pub fn acquire_dhcp(iface: &str) {
    if networkd_running() {
        tracing::debug!("systemd-networkd is active; {iface} gets its lease on its own");
        return;
    }
    if !binary_exists("pkexec") {
        tracing::warn!("pkexec missing, cannot start a DHCP client for {iface}");
        return;
    }
    // (binary, extra args): dhclient one-shot or dhcpcd background.
    let clients: [(&str, &[&str]); 2] = [("dhclient", &["-1", "-v"]), ("dhcpcd", &["-b", "-q"])];
    for (bin, args) in clients {
        if !binary_exists(bin) {
            continue;
        }
        match Command::new("pkexec")
            .arg(bin)
            .args(args)
            .arg(iface)
            .status()
        {
            Ok(status) if status.success() => return,
            Ok(status) => tracing::warn!("{bin} exited with {status}"),
            Err(err) => tracing::warn!("could not run pkexec {bin}: {err}"),
        }
    }
    tracing::info!("no DHCP client available; {iface} must be configured by networkd");
}

/// Whether systemd-networkd is running and configuring links.
fn networkd_running() -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", "systemd-networkd"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Whether `name` resolves to a file on PATH — plus the sbin dirs
/// user environments typically omit (`ufw`, `dhcpcd` live there).
pub fn binary_exists(name: &str) -> bool {
    find_binary(name).is_some()
}

/// Absolute path of `name`, searching PATH plus the sbin dirs.
pub fn find_binary(name: &str) -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in ["/usr/sbin", "/sbin", "/usr/local/sbin"] {
        dirs.push(extra.into());
    }
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Machine hostname, from `/etc/hostname` with a uname fallback.
pub fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

/// Extract `nameserver` lines, ignoring comments and options.
fn parse_resolv(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("nameserver")
                .map(|rest| rest.trim().to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_resolv;

    #[test]
    fn resolv_parses_nameservers() {
        let input = "# generated\nnameserver 192.168.1.1\nnameserver  fd00::1\noptions edns0\n";
        assert_eq!(parse_resolv(input), vec!["192.168.1.1", "fd00::1"]);
    }

    #[test]
    fn resolv_ignores_empty_file() {
        assert!(parse_resolv("").is_empty());
    }
}
