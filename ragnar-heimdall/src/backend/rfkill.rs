//! RF-kill detection for wireless interfaces.
//!
//! A soft- or hard-blocked radio is a classic silent failure mode:
//! receive still works (scans show cached/beacon results), but every
//! transmit — including association probes — is dropped by the
//! kernel. Detecting it up front turns an unexplained connect
//! timeout into an actionable "radio is switched off" message.

use anyhow::Context;
use std::fs;
use std::path::Path;

const RFKILL_ROOT: &str = "/sys/class/rfkill";

/// One rfkill switch as reported by sysfs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfKillSwitch {
    /// Switch type, e.g. `wlan`, `bluetooth`, `wwan`.
    pub kind: String,
    /// Kernel-level (firmware/BIOS switch) block.
    pub hard: bool,
    /// Userspace-toggleable block.
    pub soft: bool,
}

/// Enumerate all rfkill switches from sysfs.
fn enumerate(root: &Path) -> Vec<RfKillSwitch> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| read_switch(&e.path()))
        .collect()
}

fn read_switch(dir: &Path) -> Option<RfKillSwitch> {
    Some(RfKillSwitch {
        kind: fs::read_to_string(dir.join("type"))
            .ok()?
            .trim()
            .to_string(),
        hard: read_flag(dir, "hard"),
        soft: read_flag(dir, "soft"),
    })
}

fn read_flag(dir: &Path, name: &str) -> bool {
    fs::read_to_string(dir.join(name))
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
}

/// True when any wlan-typed switch is blocked (soft or hard).
/// `iface` is accepted for future per-phy matching; today the
/// rfkill→phy mapping is resolved through `device/` symlinks which
/// differ per driver, so a blocked wlan switch always reports
/// conservatively.
pub fn wlan_blocked(_iface: &str) -> Option<RfKillSwitch> {
    enumerate(Path::new(RFKILL_ROOT))
        .into_iter()
        .find(|s| s.kind == "wlan" && (s.hard || s.soft))
}

/// Whether any wlan switch is currently soft-blocked — used by the
/// Wi-Fi power toggle. Hard blocks are physical and can't be lifted.
pub fn wlan_soft_blocked() -> bool {
    enumerate(Path::new(RFKILL_ROOT))
        .into_iter()
        .any(|s| s.kind == "wlan" && s.soft)
}

/// Soft-block or unblock all wlan radios (`rfkill block|unblock
/// wifi`) through the privileged helper. Hard-blocked radios ignore
/// this by definition — the kernel owns those.
pub fn set_wlan_blocked(block: bool) -> anyhow::Result<()> {
    let prog = super::ufw::HELPER_PATH;
    anyhow::ensure!(
        Path::new(prog).is_file(),
        "ragnar-heimdall-helper not installed"
    );
    let output = std::process::Command::new("pkexec")
        .arg(prog)
        .args(["rfkill", "wifi", if block { "block" } else { "unblock" }])
        .output()
        .context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(());
    }
    anyhow::bail!(
        "rfkill failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

#[cfg(test)]
mod tests {
    use super::{enumerate, RfKillSwitch};
    use std::fs;

    fn write_switch(root: &std::path::Path, name: &str, kind: &str, soft: &str, hard: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("type"), kind).unwrap();
        fs::write(dir.join("soft"), soft).unwrap();
        fs::write(dir.join("hard"), hard).unwrap();
    }

    #[test]
    fn finds_blocked_wlan() {
        let dir =
            std::env::temp_dir().join(format!("ragnar-heimdall-rfkill-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write_switch(&dir, "rfkill0", "wlan", "1", "0");
        write_switch(&dir, "rfkill1", "bluetooth", "0", "0");

        let switches = enumerate(&dir);
        let blocked = switches
            .iter()
            .find(|s| s.kind == "wlan" && (s.hard || s.soft));
        assert_eq!(
            blocked,
            Some(&RfKillSwitch {
                kind: "wlan".into(),
                hard: false,
                soft: true,
            })
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unblocked_wlan_ignored() {
        let dir =
            std::env::temp_dir().join(format!("ragnar-heimdall-rfkill2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write_switch(&dir, "rfkill0", "wlan", "0", "0");

        let switches = enumerate(&dir);
        assert!(!switches
            .iter()
            .any(|s| s.kind == "wlan" && (s.hard || s.soft)));
        let _ = fs::remove_dir_all(&dir);
    }
}
