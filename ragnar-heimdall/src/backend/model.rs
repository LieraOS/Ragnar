// Copyright (C) Jörg Schröder
//! Data model describing one network interface and its addressing.

/// Classification of a link, derived from iproute2 `info_kind` and
/// sysfs markers (e.g. `/sys/class/net/<iface>/wireless`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Loopback,
    Ethernet,
    Wifi,
    Bridge,
    Bond,
    Vlan,
    Wireguard,
    Tunnel,
    Other,
}

/// Operational state reported by the kernel (`operstate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Up,
    Down,
    Unknown,
}

/// One network interface as the UI consumes it.
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub kind: LinkKind,
    pub state: LinkState,
    /// MAC address, absent for loopback-style links.
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    /// `"addr/prefix"` strings, IPv4 and IPv6 combined.
    pub addresses: Vec<String>,
    /// Administrative state from IFF_UP (switch position) and
    /// physical carrier (cable present) — operstate alone can't
    /// distinguish "admin-down" from "up but no cable".
    pub admin_up: bool,
    pub carrier: bool,
    /// Link speed in Mbit/s (sysfs), driver name, byte counters.
    pub speed_mbps: Option<u32>,
    pub driver: Option<String>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    /// Kernel IPv6 disable flag (`disable_ipv6` sysctl) — the UI
    /// switch mirrors this so "off" is a real state, not a guess.
    pub ipv6_disabled: bool,
    /// DHCP route metric from our networkd drop-in — `None` means
    /// automatic (no explicit metric configured).
    pub route_metric: Option<u32>,
}

impl NetworkInterface {
    /// True when the interface holds an IPv4 that is neither loopback
    /// nor link-local — 169.254/16 means DHCP failed or is still
    /// falling back, not a usable lease.
    pub fn has_usable_ipv4(&self) -> bool {
        self.addresses.iter().any(|a| {
            a.split('/')
                .next()
                .and_then(|ip| ip.parse::<std::net::Ipv4Addr>().ok())
                .map(|v4| !v4.is_loopback() && !v4.is_link_local())
                .unwrap_or(false)
        })
    }
}
