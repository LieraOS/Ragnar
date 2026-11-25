// Copyright (C) Jörg Schröder
//! Input validators — every argument is checked here BEFORE it can
//! reach a command line or a file path. These functions are pure
//! and fully unit-tested: the audit surface lives in this file.

/// Kernel interface names: up to IFNAMSIZ-1 chars, alnum plus the
/// characters systemd/udev names actually use (`.`, `-`, `_`).
/// Rejects anything else — no shell chars, no slashes, no escapes.
pub fn iface(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// VPN profile names: same charset, additionally rejecting leading
/// dots and `..` so the name can never become a path escape when it
/// is later composed into /etc/wireguard/<name>.conf.
pub fn profile_name(name: &str) -> bool {
    iface(name) && !name.starts_with('.') && !name.contains("..")
}

/// Digits only — for MTU, metrics, VLAN ids parsed as numbers.
pub fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// A metric argument: either a digit string or the literal `auto`.
pub fn metric(s: &str) -> bool {
    s == "auto" || digits(s)
}

/// Strict dotted-quad IPv4 — each octet parses as u8.
pub fn ipv4(s: &str) -> bool {
    let mut parts = s.split('.');
    (0..4).all(|_| {
        parts
            .next()
            .and_then(|p| p.parse::<u8>().ok())
            .is_some()
    }) && parts.next().is_none()
}

/// CIDR `a.b.c.d/p` — valid v4 plus prefix 0..=32.
pub fn cidr(s: &str) -> bool {
    let Some((ip, prefix)) = s.split_once('/') else {
        return false;
    };
    ipv4(ip) && prefix.parse::<u8>().map(|p| p <= 32).unwrap_or(false)
}

/// iwd profile file names under /var/lib/iwd — the charset iwd
/// itself uses (includes `=` for its hex escapes), confined to a
/// single path component and a known suffix. No `..`, no `/`.
pub fn iwd_file(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._=-".contains(c))
        && (name.ends_with(".psk") || name.ends_with(".open") || name.ends_with(".8021x"))
}

/// Staged VPN import path: must sit under /tmp with the Heimdall
/// prefix — never an arbitrary caller-chosen path.
pub fn staged_path(p: &str) -> bool {
    p.starts_with("/tmp/ragnar-heimdall-vpn-") && !p.contains("..") && !p.contains("//")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iface_names() {
        assert!(iface("enp2s0"));
        assert!(iface("wlan0"));
        assert!(iface("br-lan.10"));
        assert!(!iface(""));
        assert!(!iface("eth0;rm -rf"));
        assert!(!iface("../etc"));
        assert!(!iface("a".repeat(16).as_str()));
        assert!(!iface("eth 0"));
    }

    #[test]
    fn profile_names() {
        assert!(profile_name("home-wg"));
        assert!(!profile_name(".hidden"));
        assert!(!profile_name("a..b"));
        assert!(!profile_name("a/b"));
    }

    #[test]
    fn numbers() {
        assert!(digits("1500"));
        assert!(!digits("15a0"));
        assert!(!digits(""));
        assert!(metric("auto"));
        assert!(metric("100"));
        assert!(!metric("1x"));
    }

    #[test]
    fn ip_formats() {
        assert!(ipv4("192.168.1.1"));
        assert!(!ipv4("192.168.1"));
        assert!(!ipv4("192.168.1.256"));
        assert!(cidr("10.0.0.5/24"));
        assert!(!cidr("10.0.0.5/33"));
        assert!(!cidr("10.0.0.5"));
    }

    #[test]
    fn iwd_files() {
        assert!(iwd_file("HomeNet.psk"));
        assert!(iwd_file("Guest.open"));
        assert!(iwd_file("Corp.8021x"));
        assert!(!iwd_file("../evil.psk"));
        assert!(!iwd_file("a/b.psk"));
        assert!(!iwd_file("net.conf"));
        assert!(!iwd_file("net.psk;rm"));
    }

    #[test]
    fn staged_paths() {
        assert!(staged_path("/tmp/ragnar-heimdall-vpn-abc123"));
        assert!(!staged_path("/etc/passwd"));
        assert!(!staged_path("/tmp/ragnar-heimdall-vpn-x/../y"));
    }
}
