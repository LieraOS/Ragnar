// Copyright (C) Jörg Schröder
//! WiFi backend built on iwd's D-Bus API (`net.connman.iwd`).
//!
//! iwd handles scan + connect as one atomic operation — unlike
//! wpa_supplicant's select-behind-pending-scan model, a stale scan can
//! never silently starve a connect here. Networks persist themselves
//! in `/var/lib/iwd` after a successful join, and DHCP stays with
//! systemd-networkd (`EnableNetworkConfiguration` defaults off).

mod agent;
mod connect;
mod profile;
mod scan;

pub use connect::{connect, disconnect, forget, is_known, ConnectOutcome};
pub use profile::{load as profile_load, save as profile_save, Profile};
pub use scan::{dedup_by_ssid, scan};

use std::collections::HashMap;

use anyhow::{Context, Result};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, Str};

/// D-Bus service and object layout of iwd.
const SERVICE: &str = "net.connman.iwd";
const ROOT_PATH: &str = "/net/connman/iwd";
const AGENT_MGR_IFACE: &str = "net.connman.iwd.AgentManager";
const DEVICE_IFACE: &str = "net.connman.iwd.Device";
const STATION_IFACE: &str = "net.connman.iwd.Station";
const STATION_DIAG_IFACE: &str = "net.connman.iwd.StationDiagnostic";
const NETWORK_IFACE: &str = "net.connman.iwd.Network";
const KNOWN_NETWORK_IFACE: &str = "net.connman.iwd.KnownNetwork";

/// One discovered network (a BSS), already de-duplicated per SSID.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: String,
    pub signal_dbm: i16,
    pub frequency_mhz: u16,
    /// True when the BSS requires credentials (psk/8021x/owe).
    pub secured: bool,
}

/// Connect to the system bus.
fn system_conn() -> Result<Connection> {
    Connection::system().context("connect to system bus")
}

/// Find the object path of the `Device`/`Station` pair whose kernel
/// interface name is `iface`. iwd exposes both interfaces on the same
/// object (`/net/connman/iwd/<phy>/<dev>`), so one lookup is enough.
/// `GetManagedObjects` reply shape: path → interface → property map.
type ManagedObjects =
    HashMap<OwnedObjectPath, HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>>;

/// All objects iwd exposes via `GetManagedObjects` — shared by the
/// device lookup and the known-profile lookup in `connect`.
pub(crate) fn managed_objects(conn: &Connection) -> Result<ManagedObjects> {
    let mgr = Proxy::new(conn, SERVICE, "/", "org.freedesktop.DBus.ObjectManager")
        .context("iwd object manager proxy")?;
    mgr.call("GetManagedObjects", &())
        .context("list iwd managed objects")
}

fn device_path(conn: &Connection, iface: &str) -> Result<OwnedObjectPath> {
    let objects = managed_objects(conn)?;
    for (path, ifaces) in objects {
        let Some(device) = ifaces.get(DEVICE_IFACE) else {
            continue;
        };
        let name_matches = device
            .get("Name")
            .and_then(|v| v.downcast_ref::<Str>().ok())
            .map(|n| n.as_str() == iface)
            .unwrap_or(false);
        if name_matches && ifaces.contains_key(STATION_IFACE) {
            return Ok(path);
        }
    }
    anyhow::bail!("wifi interface '{iface}' is not managed by iwd");
}

/// Station proxy on a device object path.
fn station_proxy<'a>(conn: &'a Connection, path: &'a str) -> Result<Proxy<'a>> {
    Proxy::new(conn, SERVICE, path, STATION_IFACE).context("iwd station proxy")
}

/// Device proxy on a device object path (device power state).
fn device_proxy<'a>(conn: &'a Connection, path: &'a str) -> Result<Proxy<'a>> {
    Proxy::new(conn, SERVICE, path, DEVICE_IFACE).context("iwd device proxy")
}

/// `GetOrderedNetworks` returns `(object, signal)` pairs. The signal
/// width differs across iwd versions (`n` on current builds, `i` on
/// older ones) — both wire types are tried on the same reply body.
/// Note a `v` (variant) slot would NOT accept either: `v` and `n`/`i`
/// are distinct D-Bus signatures, not encodings of each other.
fn ordered_networks(station: &Proxy<'_>) -> Result<Vec<(OwnedObjectPath, i64)>> {
    let msg = station
        .call_method("GetOrderedNetworks", &())
        .context("list ordered iwd networks")?;
    let body = msg.body();
    if let Ok(pairs) = body.deserialize::<Vec<(OwnedObjectPath, i16)>>() {
        return Ok(pairs.into_iter().map(|(p, s)| (p, i64::from(s))).collect());
    }
    let pairs: Vec<(OwnedObjectPath, i32)> =
        body.deserialize().context("decode ordered iwd networks")?;
    Ok(pairs.into_iter().map(|(p, s)| (p, i64::from(s))).collect())
}

/// Power the device on when it is off — an unpowered device answers
/// scan and connect alike with `NotReady`, which reads as an empty
/// network list.
fn ensure_powered(conn: &Connection, dev_path: &str) -> Result<()> {
    let dev = device_proxy(conn, dev_path)?;
    let powered: bool = dev.get_property("Powered").unwrap_or(false);
    if !powered {
        dev.call::<_, _, ()>(
            "SetProperty",
            &("Powered", zbus::zvariant::Value::from(true)),
        )
        .context("power on wifi device")?;
    }
    Ok(())
}

/// SSID of the currently associated network, or `None` when the
/// station is not connected. Pure read of `Station.ConnectedNetwork`
/// — it can never drop or alter the association.
pub fn connected_ssid(iface: &str) -> Option<String> {
    let conn = system_conn().ok()?;
    let dev_path = device_path(&conn, iface).ok()?;
    let station = station_proxy(&conn, dev_path.as_str()).ok()?;
    let path: OwnedObjectPath = station.get_property("ConnectedNetwork").ok()?;
    // iwd reports "/" when no network is connected.
    if path.as_str() == "/" {
        return None;
    }
    let net = Proxy::new(&conn, SERVICE, path.as_str(), NETWORK_IFACE).ok()?;
    net.get_property::<String>("Name").ok()
}

/// Fail fast when the wifi radio is rfkill-blocked — a blocked card
/// scans fine but can never transmit, which reads as a dead connect.
fn rfkill_check(iface: &str) -> Result<()> {
    if let Some(sw) = super::rfkill::wlan_blocked(iface) {
        let way = if sw.hard {
            "hardware switch"
        } else {
            "airplane mode"
        };
        anyhow::bail!("wifi radio is switched off ({way}) — unblock it and retry");
    }
    Ok(())
}
