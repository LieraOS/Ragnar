//! WiFi scanning via iwd's `Station` + `Station.Diagnostic`
//! interfaces. `GetDiagnostics` yields per-BSS details (SSID, BSSID,
//! frequency, signal, security) — everything the UI needs.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Str};

use super::{
    device_path, ensure_powered, ordered_networks, station_proxy, system_conn, WifiNetwork,
    STATION_DIAG_IFACE,
};

/// Maximum time to wait for a scan to finish. On a restrictive
/// regulatory domain channels scan passively — dwell time is longer,
/// so keep headroom; iwd still delivers results when the kernel
/// finishes.
const SCAN_TIMEOUT: Duration = Duration::from_secs(45);

/// Scan `iface` and return the strongest BSS per SSID, sorted by
/// signal strength. Blocks the calling thread — run it off the UI.
pub fn scan(iface: &str) -> Result<Vec<WifiNetwork>> {
    let conn = system_conn()?;
    let dev_path = device_path(&conn, iface)?;
    // An unpowered device answers Scan with NotReady — bring it up
    // first instead of surfacing an empty network list.
    ensure_powered(&conn, dev_path.as_str())?;
    let station = station_proxy(&conn, dev_path.as_str())?;
    trigger_scan(&station)?;
    // Diagnostics carry per-BSS detail; if that interface is absent
    // or errors, the ordered-network list still shows the networks.
    // When both fail the user must see BOTH reasons — surfacing only
    // the fallback error hides the root cause (as happened on the
    // live ISO, where only the fallback's signature bug was visible).
    match collect_diagnostics(&conn, &dev_path) {
        Ok(nets) => Ok(nets),
        Err(diag_err) => {
            tracing::warn!(
                "diagnostic scan failed ({diag_err:#}), falling back to ordered networks"
            );
            collect_ordered(&conn, &dev_path).map_err(|ord_err| {
                anyhow::anyhow!("{ord_err:#} (diagnostics also failed: {diag_err:#})")
            })
        }
    }
}

/// Kick a scan and wait until `Scanning` clears. `net.connman.iwd.Busy`
/// means a scan is already in flight — that is fine, we simply wait
/// for it instead of failing.
pub(super) fn trigger_scan(station: &Proxy<'_>) -> Result<()> {
    match station.call_method("Scan", &()) {
        Ok(_) => {}
        Err(e) => {
            let busy = e.to_string().contains("net.connman.iwd.Busy");
            anyhow::ensure!(busy, "start wifi scan: {e}");
        }
    }
    let deadline = Instant::now() + SCAN_TIMEOUT;
    loop {
        let scanning: bool = station.get_property("Scanning").unwrap_or(false);
        if !scanning {
            return Ok(());
        }
        anyhow::ensure!(Instant::now() < deadline, "wifi scan timed out");
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Read `Station.Diagnostic.GetDiagnostics` and fold the per-BSS dicts
/// into networks. The diagnostic map keys are BSS addresses.
fn collect_diagnostics(
    conn: &Connection,
    dev_path: &super::OwnedObjectPath,
) -> Result<Vec<WifiNetwork>> {
    let diag = Proxy::new(conn, super::SERVICE, dev_path.as_str(), STATION_DIAG_IFACE)
        .context("iwd diagnostic proxy")?;
    let msg = diag
        .call_method("GetDiagnostics", &())
        .context("read iwd scan diagnostics")?;
    let body = msg.body();
    // Three wire shapes exist in the wild: `aa{sv}` (array of dicts,
    // BSS MAC in the "Address" field), `a{sa{sv}}` (dict keyed by
    // BSSID), and `a{sv}` (dict of variant-wrapped per-BSS dicts, as
    // emitted by the iwd shipped on this system).
    if let Ok(entries) = body.deserialize::<Vec<HashMap<String, OwnedValue>>>() {
        return Ok(dedup_by_ssid(
            entries.iter().filter_map(bss_to_network).collect(),
        ));
    }
    if let Ok(map) = body.deserialize::<HashMap<String, OwnedValue>>() {
        let raw: Vec<WifiNetwork> = map
            .iter()
            .filter_map(|(mac, wrapped)| {
                let dict = wrapped.downcast_ref::<zbus::zvariant::Dict>().ok()?;
                let mut props: HashMap<String, OwnedValue> = HashMap::new();
                for (key, value) in dict.iter() {
                    let Ok(key) = <&str>::try_from(key) else {
                        continue;
                    };
                    let Ok(value) = OwnedValue::try_from(value) else {
                        continue;
                    };
                    props.insert(key.to_string(), value);
                }
                bss_to_network(&props).map(|n| WifiNetwork {
                    bssid: if n.bssid.is_empty() {
                        mac.clone()
                    } else {
                        n.bssid
                    },
                    ..n
                })
            })
            .collect();
        if !raw.is_empty() {
            return Ok(dedup_by_ssid(raw));
        }
    }
    let map: HashMap<String, HashMap<String, OwnedValue>> =
        body.deserialize().context("decode iwd scan diagnostics")?;
    let raw: Vec<WifiNetwork> = map
        .iter()
        .filter_map(|(mac, props)| {
            bss_to_network(props).map(|n| WifiNetwork {
                bssid: if n.bssid.is_empty() {
                    mac.clone()
                } else {
                    n.bssid
                },
                ..n
            })
        })
        .collect();
    Ok(dedup_by_ssid(raw))
}

/// Fallback when the Diagnostic interface is unavailable or errors:
/// `GetOrderedNetworks` still yields every network iwd found — just
/// without per-BSS frequency/BSSID details.
fn collect_ordered(
    conn: &Connection,
    dev_path: &super::OwnedObjectPath,
) -> Result<Vec<WifiNetwork>> {
    let station = station_proxy(conn, dev_path.as_str())?;
    let ordered = ordered_networks(&station)?;
    let mut raw = Vec::new();
    for (path, signal) in ordered {
        let Ok(net) = Proxy::new(conn, super::SERVICE, path.as_str(), super::NETWORK_IFACE) else {
            continue;
        };
        let ssid: String = net.get_property("Name").unwrap_or_default();
        if ssid.is_empty() {
            continue;
        }
        let kind: String = net.get_property("Type").unwrap_or_default();
        raw.push(WifiNetwork {
            ssid,
            bssid: String::new(),
            signal_dbm: (signal / 100) as i16,
            frequency_mhz: 0,
            secured: kind != "open",
        });
    }
    Ok(dedup_by_ssid(raw))
}

/// Decode a D-Bus `ay` (byte array) variant into a string.
fn ay_to_string(v: &OwnedValue) -> Option<String> {
    let arr = v.downcast_ref::<zbus::zvariant::Array>().ok()?;
    let bytes: Vec<u8> = arr.iter().filter_map(|e| u8::try_from(e).ok()).collect();
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Decode a numeric variant — iwd reports signal as `n` and
/// frequency as `u`; accept the common int widths defensively.
fn num_i64(v: &OwnedValue) -> Option<i64> {
    v.downcast_ref::<i16>()
        .ok()
        .map(i64::from)
        .or_else(|| v.downcast_ref::<i32>().ok().map(i64::from))
        .or_else(|| v.downcast_ref::<u32>().ok().map(i64::from))
        .or_else(|| v.downcast_ref::<u16>().ok().map(i64::from))
}

/// Turn one per-BSS diagnostic dict into a `WifiNetwork`.
/// `SignalStrength` is reported in 0.01 dBm units.
fn bss_to_network(props: &HashMap<String, OwnedValue>) -> Option<WifiNetwork> {
    let ssid = ay_to_string(props.get("SSID")?)?;
    if ssid.is_empty() {
        return None;
    }
    let signal_dbm = props
        .get("SignalStrength")
        .and_then(num_i64)
        .map(|v| (v / 100) as i16)
        .unwrap_or(0);
    let frequency = props.get("Frequency").and_then(num_i64).unwrap_or(0) as u16;
    let security = props
        .get("Security")
        .and_then(|v| v.downcast_ref::<Str>().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "none".to_string());
    let bssid = props
        .get("Address")
        .and_then(|v| v.downcast_ref::<Str>().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    Some(WifiNetwork {
        ssid,
        bssid,
        signal_dbm,
        frequency_mhz: frequency,
        secured: security != "none",
    })
}

/// Keep the strongest BSS per SSID and sort descending by signal.
/// Pure and unit-testable.
pub fn dedup_by_ssid(raw: Vec<WifiNetwork>) -> Vec<WifiNetwork> {
    let mut best: HashMap<String, WifiNetwork> = HashMap::new();
    for net in raw {
        best.entry(net.ssid.clone())
            .and_modify(|n| {
                if net.signal_dbm > n.signal_dbm {
                    *n = net.clone();
                }
            })
            .or_insert(net);
    }
    let mut nets: Vec<_> = best.into_values().collect();
    nets.sort_by_key(|n| std::cmp::Reverse(n.signal_dbm));
    nets
}

#[cfg(test)]
mod tests {
    use super::{dedup_by_ssid, WifiNetwork};

    fn net(ssid: &str, dbm: i16) -> WifiNetwork {
        WifiNetwork {
            ssid: ssid.into(),
            bssid: "00:11:22:33:44:55".into(),
            signal_dbm: dbm,
            frequency_mhz: 2437,
            secured: true,
        }
    }

    #[test]
    fn dedup_keeps_strongest_per_ssid() {
        let nets = dedup_by_ssid(vec![net("Home", -70), net("Home", -45), net("Other", -60)]);
        assert_eq!(nets.len(), 2);
        let home = nets.iter().find(|n| n.ssid == "Home").unwrap();
        assert_eq!(home.signal_dbm, -45);
    }

    /// Live integration check — needs iwd running with a wireless
    /// interface. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires live iwd"]
    fn live_scan_returns_networks() {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .try_init()
            .ok();
        let nets = super::scan("wlp4s0");
        match &nets {
            Ok(list) => println!("found {} networks: {:?}", list.len(), list),
            Err(err) => println!("scan failed: {err:#}"),
        }
        assert!(nets.is_ok());
    }

    #[test]
    fn dedup_sorts_by_signal_descending() {
        let nets = dedup_by_ssid(vec![net("Weak", -90), net("Strong", -40)]);
        assert_eq!(nets[0].ssid, "Strong");
    }
}
