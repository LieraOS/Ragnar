//! VPN backend: profile management for WireGuard and OpenVPN
//! through `pkexec` + `ragnar-heimdall-helper`. Providers are irrelevant —
//! every VPN service that ships a `.conf`/`.ovpn` file works, which
//! is all of them.
//!
//! Profiles live in the standard system locations
//! (`/etc/wireguard`, `/etc/openvpn/client`), so tunnels started
//! here behave exactly like tunnels started by hand — including
//! `wg-quick@`/`openvpn-client@` systemd units for autostart.

use anyhow::{Context, Result};
use std::process::Command;

/// VPN technology a profile belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnKind {
    WireGuard,
    OpenVpn,
}

impl VpnKind {
    /// Helper's short name for the kind.
    pub fn tag(self) -> &'static str {
        match self {
            VpnKind::WireGuard => "wg",
            VpnKind::OpenVpn => "ovpn",
        }
    }

    /// Display label.
    pub fn label(self) -> &'static str {
        match self {
            VpnKind::WireGuard => "WireGuard",
            VpnKind::OpenVpn => "OpenVPN",
        }
    }
}

/// One imported profile plus its live state.
#[derive(Debug, Clone)]
pub struct VpnProfile {
    pub kind: VpnKind,
    /// Profile name = config file stem.
    pub name: String,
    /// Tunnel interface currently up.
    pub active: bool,
    /// systemd unit enabled to start at boot.
    pub autostart: bool,
    /// Received bytes while up (WireGuard only).
    pub rx_bytes: Option<u64>,
    /// Transmitted bytes while up (WireGuard only).
    pub tx_bytes: Option<u64>,
}

/// Whether WireGuard tooling exists — hides the WireGuard parts
/// when `wireguard-tools` is not installed.
pub fn wg_available() -> bool {
    super::system::binary_exists("wg-quick")
}

/// Whether OpenVPN is installed.
pub fn ovpn_available() -> bool {
    super::system::binary_exists("openvpn")
}

/// Snapshot of all profiles and their states. Goes through the
/// helper because `/etc/wireguard` and `/etc/openvpn/client` are
/// root-readable only — one polkit prompt per page visit.
pub fn profiles() -> Result<Vec<VpnProfile>> {
    let out = run_output(&["status"])?;
    Ok(parse_status(&out))
}

/// Install a config file as a named profile. `src` must be a
/// user-readable path — it is staged into /tmp under the prefix the
/// helper accepts, then moved into place with 0600 root ownership.
pub fn import_profile(kind: VpnKind, name: &str, src: &std::path::Path) -> Result<()> {
    validate_name(name)?;
    anyhow::ensure!(src.is_file(), "config file not found");

    // Stage into /tmp so the helper can verify the origin directory.
    let staged = std::path::PathBuf::from(format!(
        "/tmp/ragnar-heimdall-vpn-{}-{}",
        std::process::id(),
        name
    ));
    std::fs::copy(src, &staged).context("could not stage config into /tmp")?;
    let result = run(&[
        "import",
        kind.tag(),
        name,
        staged.to_str().unwrap_or_default(),
    ]);
    if result.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    result.with_context(|| format!("import {} profile", kind.label()))
}

/// Bring a profile's tunnel up or down.
pub fn set_up(profile: &VpnProfile, up: bool) -> Result<()> {
    run(&[
        if up { "up" } else { "down" },
        profile.kind.tag(),
        &profile.name,
    ])
    .with_context(|| format!("{} {}", if up { "start" } else { "stop" }, profile.name))
}

/// Remove the profile: tunnel down, autostart off, config deleted.
pub fn delete_profile(profile: &VpnProfile) -> Result<()> {
    run(&["delete", profile.kind.tag(), &profile.name])
        .with_context(|| format!("delete {}", profile.name))
}

/// Enable/disable the boot-time systemd unit of the profile.
pub fn set_autostart(profile: &VpnProfile, on: bool) -> Result<()> {
    run(&[
        "autostart",
        profile.kind.tag(),
        &profile.name,
        if on { "on" } else { "off" },
    ])
    .with_context(|| format!("autostart {}", profile.name))
}

/// `ip` already lives in PATH on the dev box; the helper requires
/// an explicit tool prefix.
fn run(args: &[&str]) -> Result<()> {
    run_output(args).map(|_| ())
}

/// Execute `vpn <args>` through pkexec + helper and capture stdout.
fn run_output(args: &[&str]) -> Result<String> {
    let prog = if std::path::Path::new(super::ufw::HELPER_PATH).is_file() {
        super::ufw::HELPER_PATH.to_string()
    } else {
        anyhow::bail!("ragnar-heimdall-helper not installed — install the ragnar-heimdall package to manage VPNs")
    };
    let mut call = Command::new("pkexec");
    call.arg(&prog).arg("vpn").args(args);
    let output = call.output().context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("vpn helper failed ({}): {}", output.status, stderr)
}

/// Parse the helper's `kind|name|state|autostart|rx|tx` lines.
fn parse_status(out: &str) -> Vec<VpnProfile> {
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('|');
            let kind = match f.next()? {
                "wg" => VpnKind::WireGuard,
                "ovpn" => VpnKind::OpenVpn,
                _ => return None,
            };
            let name = f.next()?.to_string();
            let active = f.next()? == "up";
            let autostart = f.next().unwrap_or("") == "enabled";
            let rx_bytes = f.next().and_then(|s| s.parse().ok());
            let tx_bytes = f.next().and_then(|s| s.parse().ok());
            Some(VpnProfile {
                kind,
                name,
                active,
                autostart,
                rx_bytes,
                tx_bytes,
            })
        })
        .collect()
}

/// Profile names become file names and systemd unit names — keep
/// them to a strict charset so no path or unit injection is possible.
fn validate_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
            && !name.starts_with('.'),
        "profile name may only contain letters, digits, '.', '_' and '-'"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parses_helper_lines() {
        let out = "wg|home|up|enabled|1048576|524288\n\
                   ovpn|work|down|disabled||\n";
        let profiles = parse_status(out);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].kind, VpnKind::WireGuard);
        assert!(profiles[0].active);
        assert!(profiles[0].autostart);
        assert_eq!(profiles[0].rx_bytes, Some(1048576));
        assert_eq!(profiles[1].kind, VpnKind::OpenVpn);
        assert!(!profiles[1].active);
        assert_eq!(profiles[1].rx_bytes, None);
    }

    #[test]
    fn names_reject_injection() {
        assert!(validate_name("home-vpn_2").is_ok());
        assert!(validate_name("../etc").is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name(".hidden").is_err());
    }
}
