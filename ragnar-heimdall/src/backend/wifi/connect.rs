// Copyright (C) Jörg Schröder
//! WiFi connect flow via iwd. `Network.Connect` runs scan, associate,
//! authenticate and the 4-way handshake internally as one operation —
//! there is no select-behind-pending-scan window like in
//! wpa_supplicant, which is what produced the silent live-ISO timeouts.
//! Credentials arrive through the registered passphrase agent; on
//! success iwd persists the profile under `/var/lib/iwd` itself.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, Str};

use super::{
    agent, device_path, ensure_powered, ordered_networks, rfkill_check, station_proxy, system_conn,
    WifiNetwork, KNOWN_NETWORK_IFACE, NETWORK_IFACE, SERVICE,
};

/// Maximum time to wait for iwd to finish associating after
/// `Network.Connect`. iwd retries internally; a wrong passphrase ends
/// as `connecting -> disconnected` well inside this window.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum time to wait for a DHCP lease after iwd reports
/// `connected` — systemd-networkd typically lands it within seconds.
const IP_TIMEOUT: Duration = Duration::from_secs(15);

/// Outcome of a successful connect.
#[derive(Debug)]
pub struct ConnectOutcome {
    /// Whether iwd stored the profile in `/var/lib/iwd` — i.e. the
    /// connection survives reboots.
    pub persisted: bool,
    /// Whether the interface received a usable IPv4 address within
    /// the wait window. `false` means "joined but no internet yet".
    pub ip_acquired: bool,
}

/// Connect `iface` to `net`.
///
/// With a fresh passphrase the network's stored credentials (if any)
/// are forgotten first so a changed password actually takes effect;
/// iwd then asks the agent for the passphrase during `Connect`.
/// Without a passphrase the stored profile is reused — for a secured
/// network that never connected before this fails fast instead of
/// timing out.
pub fn connect(iface: &str, net: &WifiNetwork, password: Option<&str>) -> Result<ConnectOutcome> {
    // A blocked radio still scans (cached results) but can never
    // transmit — association included. Fail fast with a clear reason.
    rfkill_check(iface)?;

    let conn = system_conn()?;
    let dev_path = device_path(&conn, iface)?;
    let station = station_proxy(&conn, dev_path.as_str())?;

    // A powered-off device answers scan/connect with NotReady —
    // bring it up explicitly (soft rfkill already excluded above).
    ensure_powered(&conn, dev_path.as_str())?;

    // Fresh scan so the Network object exists and the BSS data is
    // current — then resolve the object path for this SSID.
    super::scan::trigger_scan(&station)?;
    let net_path = find_network(&conn, &dev_path, &net.ssid)
        .with_context(|| format!("network '{}' not in scan results", net.ssid))?;
    let net_proxy = Proxy::new(&conn, SERVICE, net_path.as_str(), NETWORK_IFACE)
        .context("iwd network proxy")?;

    let passphrase = password.filter(|p| !p.is_empty());
    let known = known_network(&net_proxy);
    match (passphrase, &known) {
        // New passphrase on a known network: drop the stored profile
        // so iwd re-asks the agent instead of silently reusing the
        // old credentials.
        (Some(_), Some(known_path)) => forget_network(&conn, known_path)?,
        // Secured network, no passphrase, nothing stored → can never
        // join. Error out immediately instead of timing out.
        (None, None) if net.secured => {
            anyhow::bail!("'{}' is secured — a password is required", net.ssid)
        }
        _ => {}
    }

    agent::register_agent(&conn, passphrase.map(str::to_owned))?;
    let result = do_connect(&station, &net_proxy, &net.ssid);
    agent::unregister_agent(&conn);
    result?;

    // After a successful join iwd writes the profile itself — check
    // it actually happened so the UI can warn when persistence failed.
    let persisted = known_network(&net_proxy).is_some() || known.is_some();
    let ip_acquired = wait_for_ip(iface);
    if !ip_acquired {
        tracing::warn!("no IPv4 lease on {iface} within {}s", IP_TIMEOUT.as_secs());
    }
    Ok(ConnectOutcome {
        persisted,
        ip_acquired,
    })
}

/// Drop the current association. iwd keeps the stored profile — a
/// disconnect ends this session only, nothing is forgotten, so the
/// next connect reuses the saved credentials.
pub fn disconnect(iface: &str) -> Result<()> {
    let conn = system_conn()?;
    let dev_path = device_path(&conn, iface)?;
    let station = station_proxy(&conn, dev_path.as_str())?;
    station
        .call::<_, _, ()>("Disconnect", &())
        .context("wifi disconnect")
}

/// Issue `Network.Connect` and poll `Station.State` until iwd reaches
/// `connected`. `connecting -> disconnected` without ever connecting
/// means the AP rejected us — almost always a wrong passphrase.
fn do_connect(station: &Proxy<'_>, net_proxy: &Proxy<'_>, ssid: &str) -> Result<()> {
    net_proxy
        .call_method("Connect", &())
        .context("start wifi connect")?;
    tracing::info!("connect to '{ssid}' started, waiting for iwd state");

    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut saw_connecting = false;
    let mut last_state = String::new();
    loop {
        let state: String = station.get_property("State").unwrap_or_default();
        if state != last_state {
            tracing::info!("iwd state: '{last_state}' -> '{state}'");
            last_state = state.clone();
        }
        match state.as_str() {
            "connected" => return Ok(()),
            "disconnected" | "autoconnect_quick" | "autoconnect_full" if saw_connecting => {
                anyhow::bail!("authentication to '{ssid}' failed — check the password")
            }
            "connecting" | "roaming" => saw_connecting = true,
            _ => {}
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "wifi connect to '{ssid}' timed out (iwd stayed '{state}')"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Locate the `Network` object for `ssid` in the station's ordered
/// network list (populated by the preceding scan).
fn find_network(
    conn: &Connection,
    dev_path: &OwnedObjectPath,
    ssid: &str,
) -> Result<OwnedObjectPath> {
    let station = station_proxy(conn, dev_path.as_str())?;
    let ordered = ordered_networks(&station)?;
    for (path, _signal) in ordered {
        let proxy = Proxy::new(conn, SERVICE, path.as_str(), NETWORK_IFACE);
        let name: Option<String> = proxy.ok().and_then(|p| p.get_property("Name").ok());
        if name.as_deref() == Some(ssid) {
            return Ok(path);
        }
    }
    anyhow::bail!("ssid '{ssid}' not found");
}

/// Path of the network's `KnownNetwork` object when iwd already holds
/// stored credentials for it, else `None`.
fn known_network(net_proxy: &Proxy<'_>) -> Option<OwnedObjectPath> {
    let path: OwnedObjectPath = net_proxy.get_property("KnownNetwork").ok()?;
    let empty: OwnedObjectPath = OwnedObjectPath::try_from("/").ok()?;
    (path != empty).then(|| {
        tracing::info!("known iwd profile present: {path}");
        path
    })
}

/// Ask iwd to forget a stored profile so a new passphrase can replace it.
fn forget_network(conn: &Connection, known_path: &OwnedObjectPath) -> Result<()> {
    let known = Proxy::new(conn, SERVICE, known_path.as_str(), KNOWN_NETWORK_IFACE)
        .context("iwd known-network proxy")?;
    known
        .call::<_, _, ()>("Forget", &())
        .context("forget stored wifi profile")
}

/// Whether iwd stores a profile for `ssid` — drives the "Forget"
/// affordance in the UI.
pub fn is_known(ssid: &str) -> bool {
    known_path_for(ssid).is_some()
}

/// Forget the stored profile for `ssid` entirely — credentials,
/// autoconnect, everything iwd persisted.
pub fn forget(ssid: &str) -> Result<()> {
    let conn = system_conn()?;
    let Some(path) = known_path_for(ssid) else {
        anyhow::bail!("no stored profile for '{ssid}'");
    };
    forget_network(&conn, &path)
}

/// Find the `KnownNetwork` object path whose `Name` matches `ssid`
/// by scanning iwd's managed objects.
fn known_path_for(ssid: &str) -> Option<OwnedObjectPath> {
    let conn = system_conn().ok()?;
    let objects = super::managed_objects(&conn).ok()?;
    for (path, ifaces) in objects {
        let Some(props) = ifaces.get(KNOWN_NETWORK_IFACE) else {
            continue;
        };
        let matches = props
            .get("Name")
            .and_then(|v| v.downcast_ref::<Str>().ok())
            .map(|n| n.as_str() == ssid)
            .unwrap_or(false);
        if matches {
            return OwnedObjectPath::try_from(path.as_str()).ok();
        }
    }
    None
}

/// Wait until `iface` carries a usable IPv4 address. A successful
/// association only proves layer-2; the DHCP lease from
/// systemd-networkd lands a few seconds later. The deadline is soft —
/// a late lease still works, it just was not confirmed here.
fn wait_for_ip(iface: &str) -> bool {
    let deadline = Instant::now() + IP_TIMEOUT;
    loop {
        if has_usable_ipv4(iface) || Instant::now() >= deadline {
            return has_usable_ipv4(iface);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Whether `iface` currently has an IPv4 address that is neither
/// loopback nor link-local — `169.254/16` means DHCP failed or is
/// still falling back, not that the link is usable.
fn has_usable_ipv4(iface: &str) -> bool {
    super::super::collector::collect_interfaces()
        .map(|ifaces| {
            ifaces
                .iter()
                .find(|i| i.name == iface)
                .map(|i| i.has_usable_ipv4())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}
