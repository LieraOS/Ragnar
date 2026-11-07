//! iwd configuration options: reads whitelisted `[General]` keys
//! from `/etc/iwd/main.conf` directly (the file is world-readable)
//! and writes them through `pkexec` + `ragnar-heimdall-helper`, which also
//! restarts iwd so the option takes effect immediately.

use anyhow::{Context, Result};
use std::process::Command;

/// iwd's main.conf path — LieraOS ships exactly this file.
const MAIN_CONF: &str = "/etc/iwd/main.conf";

/// Read one `[General]` key as configured in main.conf. Returns
/// `None` when the key is not set — iwd then uses its built-in
/// default (for `AddressRandomization`: `disabled`).
pub fn get(key: &str) -> Option<String> {
    let content = std::fs::read_to_string(MAIN_CONF).ok()?;
    let mut in_general = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_general = line == "[General]";
            continue;
        }
        if in_general {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Write a whitelisted `[General]` key and restart iwd through the
/// helper. Values are validated client-side too so a programming
/// error never reaches pkexec.
pub fn set(key: &str, value: &str) -> Result<()> {
    anyhow::ensure!(
        matches!(
            (key, value),
            ("AddressRandomization", "disabled")
                | ("AddressRandomization", "once")
                | ("AddressRandomization", "network")
                | ("AddressRandomizationRange", "nic")
                | ("AddressRandomizationRange", "full")
        ),
        "unsupported iwd option {key}={value}"
    );
    let prog = if std::path::Path::new(super::ufw::HELPER_PATH).is_file() {
        super::ufw::HELPER_PATH.to_string()
    } else {
        anyhow::bail!("ragnar-heimdall-helper not installed")
    };
    let output = Command::new("pkexec")
        .arg(&prog)
        .args(["iwd", "set", key, value])
        .output()
        .context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("iwd config failed ({}): {}", output.status, stderr)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_unlisted_options() {
        assert!(super::set("EnableNetworkConfiguration", "true").is_err());
        assert!(super::set("AddressRandomization", "maybe").is_err());
    }
}
