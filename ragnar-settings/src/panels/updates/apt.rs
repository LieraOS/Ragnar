// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Updates" panel: apt queries + drop-in rendering
// Copyright (C) 2026 LieraOS Project
//
// Everything in this module is pure or read-only: counting pending
// upgrades via `apt-get -s` (a simulation needs no root and no
// lock), reading the last-check timestamp, and rendering the
// auto-upgrade drop-in. The privileged write lives in auto.rs.

use std::process::Command;
use std::time::SystemTime;

/// The drop-in this panel manages. Sorts after the distro's
/// 50unattended-upgrades so its directives win.
pub const DROPIN_PATH: &str = "/etc/apt/apt.conf.d/52ragnar-auto-upgrades";

/// Count lines starting with "Inst " in `apt-get -s dist-upgrade`
/// output — that prefix marks every package apt would install or
/// upgrade. Untranslated by apt, so it is locale-proof (the fixture
/// test below runs against real German output).
pub fn parse_update_count(sim_output: &str) -> usize {
    sim_output
        .lines()
        .filter(|l| l.starts_with("Inst "))
        .count()
}

/// Number of pending upgrades according to a non-destructive
/// simulation. Needs no root; Debug::NoLocking avoids the apt lock.
/// A failed simulation (broken lists, apt error) is an error, not
/// "0 updates" — the UI must not lie about the system state.
pub fn count_updates() -> Result<usize, String> {
    let out = Command::new("apt-get")
        .args(["-s", "-o", "Debug::NoLocking=1", "dist-upgrade"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("apt simulation failed")
            .trim()
            .to_string());
    }
    Ok(parse_update_count(&String::from_utf8_lossy(&out.stdout)))
}

/// Whether the unattended-upgrades script is installed.
pub fn unattended_upgrades_installed() -> bool {
    gtk4::glib::find_program_in_path("unattended-upgrade").is_some()
}

/// Whether automatic upgrades are currently enabled: the drop-in
/// exists and explicitly enables APT::Periodic::Unattended-Upgrade.
/// Any other state (missing file, older content) reads as off.
pub fn auto_enabled() -> bool {
    std::fs::read_to_string(DROPIN_PATH)
        .map(|s| s.contains("APT::Periodic::Unattended-Upgrade \"1\""))
        .unwrap_or(false)
}

/// Render the managed drop-in for `enabled` (true = auto-install
/// Debian updates, false = explicitly off).
///
/// Why our own Origins-Pattern: the package's shipped
/// 50unattended-upgrades uses ${distro_codename}, resolved at runtime
/// via `lsb_release -cs` — which prints "n/a" on Liera (os-release
/// has no VERSION_CODENAME), so the stock patterns match nothing and
/// auto-update would silently do nothing. Our pattern pins
/// origin=Debian,label=Debian instead: release-agnostic, covers the
/// main archive incl. -updates, and excludes every third-party repo
/// (Brave, deb-multimedia, VS Code carry different labels/origins).
/// Verified with apt-config: the #clear directive empties the
/// inherited list so exactly this one entry remains.
pub fn render_dropin(enabled: bool) -> &'static str {
    if enabled {
        "// Managed by Ragnar Settings — do not edit by hand.\n\
         // Enables unattended upgrades restricted to the Debian archive.\n\
         // The shipped 50unattended-upgrades patterns expand\n\
         // ${distro_codename} via lsb_release -cs, which prints \"n/a\" on\n\
         // Liera (no VERSION_CODENAME) and would match nothing.\n\
         APT::Periodic::Update-Package-Lists \"1\";\n\
         APT::Periodic::Unattended-Upgrade \"1\";\n\
         #clear Unattended-Upgrade::Origins-Pattern;\n\
         Unattended-Upgrade::Origins-Pattern {\n\
             \"origin=Debian,label=Debian\";\n\
         };\n"
    } else {
        "// Managed by Ragnar Settings — do not edit by hand.\n\
         // Automatic upgrades explicitly disabled; written so this file\n\
         // overrides any distro-shipped 20auto-upgrades defaults.\n\
         APT::Periodic::Update-Package-Lists \"1\";\n\
         APT::Periodic::Unattended-Upgrade \"0\";\n"
    }
}

/// "Last checked" in human wording: "today at 14:05", "yesterday at
/// 09:12", or an absolute date — from the mtime of apt's
/// update-success stamp, falling back to the lists directory.
/// Returns None when apt has never refreshed lists here.
pub fn last_checked() -> Option<String> {
    let stamp = [
        "/var/lib/apt/periodic/update-success-stamp",
        "/var/lib/apt/lists",
    ]
    .iter()
    .filter_map(|p| std::fs::metadata(p).ok())
    .filter_map(|m| m.modified().ok())
    .max()?;
    let secs = stamp.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64;
    let then = gtk4::glib::DateTime::from_unix_local(secs).ok()?;
    let now = gtk4::glib::DateTime::now_local().ok()?;
    let time = then.format("%H:%M").ok()?;
    let days = now.difference(&then).as_days();
    if days == 0 {
        Some(gettextrs::gettext("today at") + " " + &time)
    } else if days == 1 {
        Some(gettextrs::gettext("yesterday at") + " " + &time)
    } else {
        then.format("%d.%m.%Y").ok().map(|d| d.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `apt-get -s -o Debug::NoLocking=1 dist-upgrade` output
    /// captured on this machine (German locale — proves the "Inst "
    /// prefix survives localisation): 15 pending upgrades.
    #[test]
    fn count_parses_real_apt_output() {
        let fixture = include_str!("../../../tests/fixtures/apt_dist_upgrade_sim.txt");
        assert_eq!(parse_update_count(fixture), 15);
        // Empty / up-to-date output yields zero, not an error.
        assert_eq!(parse_update_count("Reading package lists...\n"), 0);
    }

    /// The enabled drop-in clears the inherited patterns and pins
    /// exactly the Debian archive; the disabled body writes "0".
    #[test]
    fn dropin_bodies() {
        let on = render_dropin(true);
        assert!(on.contains("#clear Unattended-Upgrade::Origins-Pattern;"));
        assert!(on.contains("\"origin=Debian,label=Debian\";"));
        assert!(on.contains("APT::Periodic::Unattended-Upgrade \"1\";"));
        assert!(on.contains("APT::Periodic::Update-Package-Lists \"1\";"));
        let off = render_dropin(false);
        assert!(off.contains("APT::Periodic::Unattended-Upgrade \"0\";"));
        assert!(!off.contains("#clear"));
    }
}
