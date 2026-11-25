// Copyright (C) Jörg Schröder
//! Collects live interface state. `ip -json` gives links, states and
//! addresses in a stable machine-readable form; sysfs tells us which
//! links are wireless (`ip` does not tag them).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::model::{LinkKind, LinkState, NetworkInterface};

/// sysfs root for per-interface attributes.
const SYSFS_NET: &str = "/sys/class/net";

/// `ip -j link show` record.
#[derive(Debug, Deserialize)]
struct IpLink {
    ifname: String,
    #[serde(default)]
    link_type: Option<String>,
    #[serde(default)]
    linkinfo: Option<IpLinkInfo>,
}

#[derive(Debug, Deserialize)]
struct IpLinkInfo {
    #[serde(default)]
    info_kind: Option<String>,
}

/// `ip -j address show` record.
#[derive(Debug, Deserialize)]
struct IpAddr {
    ifname: String,
    #[serde(default)]
    operstate: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    mtu: Option<u32>,
    #[serde(default)]
    addr_info: Vec<IpAddrInfo>,
}

#[derive(Debug, Deserialize)]
struct IpAddrInfo {
    local: String,
    prefixlen: u32,
}

/// Snapshot every interface the kernel knows about.
pub fn collect_interfaces() -> Result<Vec<NetworkInterface>> {
    let links: Vec<IpLink> = run_json(&["-j", "link", "show"])?;
    let addrs: Vec<IpAddr> = run_json(&["-j", "address", "show"])?;

    let mut out = Vec::with_capacity(links.len());
    for link in links {
        if link.ifname == "lo" {
            out.push(loopback());
            continue;
        }
        let info = addrs.iter().find(|a| a.ifname == link.ifname);
        out.push(NetworkInterface {
            kind: classify(&link),
            state: info
                .and_then(|a| a.operstate.as_deref())
                .map(parse_state)
                .unwrap_or(LinkState::Unknown),
            mac: info.and_then(|a| a.address.clone()),
            mtu: info.and_then(|a| a.mtu),
            addresses: info
                .map(|a| {
                    a.addr_info
                        .iter()
                        .map(|i| format!("{}/{}", i.local, i.prefixlen))
                        .collect()
                })
                .unwrap_or_default(),
            admin_up: sysfs_num(&link.ifname, "flags")
                .map(|f| f & 1 == 1)
                .unwrap_or(false),
            carrier: sysfs_num(&link.ifname, "carrier") == Some(1),
            speed_mbps: sysfs_num(&link.ifname, "speed").map(|n| n as u32),
            driver: sysfs_driver(&link.ifname),
            rx_bytes: sysfs_num(&link.ifname, "statistics/rx_bytes"),
            tx_bytes: sysfs_num(&link.ifname, "statistics/tx_bytes"),
            ipv6_disabled: std::fs::read_to_string(format!(
                "/proc/sys/net/ipv6/conf/{}/disable_ipv6",
                link.ifname
            ))
            .map(|v| v.trim() == "1")
            .unwrap_or(false),
            route_metric: route_metric(&link.ifname),
            name: link.ifname,
        });
    }
    Ok(out)
}

/// Run `ip` with the given arguments and parse its JSON array output.
fn run_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> Result<Vec<T>> {
    let output = Command::new("ip")
        .args(args)
        .output()
        .context("failed to run `ip`")?;
    anyhow::ensure!(
        output.status.success(),
        "`ip {}` exited with {}",
        args.join(" "),
        output.status
    );
    serde_json::from_slice(&output.stdout).context("failed to parse `ip -json` output")
}

/// sysfs numeric leaf under /sys/class/net/<iface>/.
fn sysfs_num(iface: &str, leaf: &str) -> Option<u64> {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/{leaf}"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Kernel driver name via the /sys/class/net/<iface>/device/driver link.
fn sysfs_driver(iface: &str) -> Option<String> {
    let p = std::fs::read_link(format!("/sys/class/net/{iface}/device/driver")).ok()?;
    Some(p.file_name()?.to_string_lossy().into_owned())
}

/// Route metric from our networkd drop-in
/// `/etc/systemd/network/10-ragnar-heimdall-<iface>.network.d/90-metric.conf`
/// — world-readable, so no privilege needed to display it.
fn route_metric(iface: &str) -> Option<u32> {
    let text = std::fs::read_to_string(format!(
        "/etc/systemd/network/10-ragnar-heimdall-{iface}.network.d/90-metric.conf"
    ))
    .ok()?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("RouteMetric="))
        .and_then(|v| v.trim().parse().ok())
}

/// Determine the link kind. `info_kind` covers virtual devices;
/// wireless links are tagged via sysfs instead.
fn classify(link: &IpLink) -> LinkKind {
    let kind = link.linkinfo.as_ref().and_then(|i| i.info_kind.as_deref());
    match kind {
        Some("bridge") => LinkKind::Bridge,
        Some("bond") => LinkKind::Bond,
        Some("vlan") => LinkKind::Vlan,
        Some("wireguard") => LinkKind::Wireguard,
        Some("gre" | "gretap" | "ipip" | "sit" | "vxlan" | "tun" | "tap") => LinkKind::Tunnel,
        Some(_) => LinkKind::Other,
        None => {
            if Path::new(SYSFS_NET)
                .join(&link.ifname)
                .join("wireless")
                .exists()
            {
                LinkKind::Wifi
            } else if link.link_type.as_deref() == Some("ether") {
                LinkKind::Ethernet
            } else {
                LinkKind::Other
            }
        }
    }
}

/// Map kernel `operstate` text to the model state.
fn parse_state(raw: &str) -> LinkState {
    match raw {
        "UP" | "up" => LinkState::Up,
        "DOWN" | "down" => LinkState::Down,
        _ => LinkState::Unknown,
    }
}

/// Loopback is always present and always up from the UI's view.
fn loopback() -> NetworkInterface {
    NetworkInterface {
        name: "lo".into(),
        kind: LinkKind::Loopback,
        state: LinkState::Up,
        mac: None,
        mtu: None,
        addresses: vec!["127.0.0.1/8".into(), "::1/128".into()],
        admin_up: true,
        carrier: true,
        speed_mbps: None,
        driver: None,
        rx_bytes: None,
        tx_bytes: None,
        ipv6_disabled: false,
        route_metric: None,
    }
}
