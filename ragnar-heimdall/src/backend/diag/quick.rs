//! The quick battery — everyday checks: link states, routing and
//! gateway reachability, DNS, services, Wi-Fi and the internet
//! speed suite. Called by [`super::run`] for enabled suite ids.

use std::process::Command;

use super::{Check, Context, Status};

/// Every non-loopback interface: state plus *all* assigned
/// addresses — machines with several NICs or stacked IPs must not
/// hide any of them.
pub(super) fn suite_links(ctx: &Context) -> Vec<Check> {
    let mut out = Vec::new();
    let mut any_up = false;
    for iface in &ctx.ifaces {
        if matches!(iface.kind, crate::backend::model::LinkKind::Loopback) {
            continue;
        }
        let up = matches!(iface.state, crate::backend::model::LinkState::Up);
        any_up |= up;
        let addrs = iface.addresses.join(", ");
        let (status, detail) = match (up, iface.has_usable_ipv4()) {
            (false, _) => (
                Status::Warn,
                format!(
                    "down — {}",
                    if addrs.is_empty() {
                        "no address"
                    } else {
                        &addrs
                    }
                ),
            ),
            (true, false) => (
                Status::Warn,
                format!(
                    "up, no usable IPv4 — {}",
                    if addrs.is_empty() {
                        "no address"
                    } else {
                        &addrs
                    }
                ),
            ),
            (true, true) => (Status::Pass, format!("up — {addrs}")),
        };
        out.push(Check::new("Links", &iface.name, status, detail));
    }
    if !any_up {
        out.push(Check::new(
            "Links",
            "physical link",
            Status::Fail,
            "no interface is up",
        ));
    }
    out
}

/// Default route presence plus gateway reachability — the ping now
/// reports real latency and packet loss instead of a bare flag.
pub(super) fn suite_routing(ctx: &Context) -> Vec<Check> {
    let mut out = Vec::new();
    match ctx.route.as_deref() {
        Some(r) if !r.is_empty() => {
            out.push(Check::new("Routing", "default route", Status::Pass, r))
        }
        _ => out.push(Check::new(
            "Routing",
            "default route",
            Status::Fail,
            "no default gateway configured",
        )),
    }
    // IPv6 default route — dual-stack machines break silently when
    // it is missing while IPv4 still works.
    let has_v6 = Command::new("ip")
        .args(["-6", "route", "show", "default"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    out.push(Check::new(
        "Routing",
        "IPv6 default route",
        if has_v6 { Status::Pass } else { Status::Warn },
        if has_v6 {
            "present"
        } else {
            "no IPv6 default route"
        },
    ));
    if let Some(gw) = ctx
        .route
        .as_deref()
        .and_then(|r| r.split_whitespace().next().map(str::to_string))
    {
        out.push(match ping_stats(&gw) {
            Some((0, avg_ms)) => Check::new(
                "Routing",
                "gateway reachable",
                Status::Pass,
                format!("{avg_ms:.1} ms avg · 0% loss"),
            ),
            Some((loss, avg_ms)) => Check::new(
                "Routing",
                "gateway reachable",
                if loss < 100 {
                    Status::Warn
                } else {
                    Status::Fail
                },
                format!("{avg_ms:.1} ms avg · {loss}% loss"),
            ),
            None => Check::new(
                "Routing",
                "gateway reachable",
                Status::Fail,
                "gateway did not answer within 4s",
            ),
        });
    }
    out
}

/// Nameserver configuration plus a real resolution test through the
/// full NSS stack — getent exercises resolv.conf, resolved or
/// whatever the system uses.
pub(super) fn suite_dns(ctx: &Context) -> Vec<Check> {
    let mut out = Vec::new();
    let servers = crate::backend::system::dns_servers();
    out.push(if servers.is_empty() {
        Check::new(
            "DNS",
            "nameservers",
            Status::Fail,
            "no nameserver configured in /etc/resolv.conf",
        )
    } else {
        Check::new("DNS", "nameservers", Status::Pass, servers.join(", "))
    });
    out.push(if !ctx.gateway_reachable {
        Check::new(
            "DNS",
            "resolution",
            Status::Skip,
            "no reachable gateway — resolution not tested",
        )
    } else if !crate::backend::system::binary_exists("getent") {
        Check::new("DNS", "resolution", Status::Skip, "getent not installed")
    } else {
        let ok = Command::new("getent")
            .args(["ahostsv4", "debian.org"])
            .output()
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false);
        Check::new(
            "DNS",
            "resolution",
            if ok { Status::Pass } else { Status::Fail },
            if ok {
                "debian.org resolves"
            } else {
                "could not resolve debian.org"
            },
        )
    });
    out
}

/// systemd unit states — networkd/iwd/ufw health without privileges.
pub(super) fn suite_services() -> Vec<Check> {
    [
        ("systemd-networkd", "networkd"),
        ("iwd", "iwd"),
        ("ufw", "ufw"),
    ]
    .iter()
    .map(|(unit, label)| {
        match Command::new("systemctl")
            .args(["is-active", "--quiet", unit])
            .status()
        {
            Ok(s) if s.success() => {
                Check::new("Services", label, Status::Pass, format!("{unit} is active"))
            }
            Ok(_) => Check::new(
                "Services",
                label,
                Status::Warn,
                format!("{unit} is not active"),
            ),
            Err(err) => Check::new(
                "Services",
                label,
                Status::Skip,
                format!("systemctl failed: {err}"),
            ),
        }
    })
    .collect()
}

/// WiFi radio state and association, per wireless interface.
pub(super) fn suite_wifi(ctx: &Context) -> Vec<Check> {
    let mut out = Vec::new();
    let wifi_ifaces: Vec<_> = ctx
        .ifaces
        .iter()
        .filter(|i| matches!(i.kind, crate::backend::model::LinkKind::Wifi))
        .collect();
    if wifi_ifaces.is_empty() {
        return vec![Check::new(
            "WiFi",
            "wifi",
            Status::Skip,
            "no wireless interface present",
        )];
    }
    for iface in wifi_ifaces {
        // `wlan_blocked` returns Some only when a wlan switch is
        // blocked — None means the radio is clear (or no switch).
        match crate::backend::rfkill::wlan_blocked(&iface.name) {
            Some(blocked) => out.push(Check::new(
                "WiFi",
                &format!("{} radio", iface.name),
                Status::Fail,
                if blocked.hard {
                    "blocked by hardware switch"
                } else {
                    "blocked in software (airplane mode)"
                },
            )),
            None => out.push(Check::new(
                "WiFi",
                &format!("{} radio", iface.name),
                Status::Pass,
                "radio enabled",
            )),
        }
        match crate::backend::wifi::connected_ssid(&iface.name) {
            Some(ssid) => out.push(Check::new(
                "WiFi",
                &format!("{} association", iface.name),
                Status::Pass,
                format!("connected to \"{ssid}\""),
            )),
            None => out.push(Check::new(
                "WiFi",
                &format!("{} association", iface.name),
                Status::Warn,
                "not associated",
            )),
        }
        // Live link signal from `iw dev <if> link` — explains slow
        // speeds the speedtest would only hint at.
        if let Some(signal) = wifi_signal(&iface.name) {
            out.push(Check::new(
                "WiFi",
                &format!("{} signal", iface.name),
                if signal > -70 {
                    Status::Pass
                } else {
                    Status::Warn
                },
                format!("{signal} dBm"),
            ));
        }
    }
    out
}

/// Current link signal in dBm via `iw dev <if> link` — `signal:
/// -52 dBm` on the associated line.
fn wifi_signal(iface: &str) -> Option<i32> {
    let out = Command::new("iw")
        .args(["dev", iface, "link"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("signal:"))
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
}

/// Internet suite: delegated to the speed module — external IP,
/// latency, download/upload throughput via HTTP measurements.
pub(super) fn suite_internet(ctx: &Context) -> Vec<Check> {
    if !ctx.gateway_reachable {
        return vec![Check::new(
            "Internet",
            "internet",
            Status::Skip,
            "no gateway reachable — internet tests skipped",
        )];
    }
    crate::backend::speed::run_all()
}

/// Single ICMP echo with a short timeout. Needs the `ping` binary —
/// returns false when it is absent.
pub(super) fn ping(host: &str) -> bool {
    ping_stats(host).is_some()
}

/// Four ICMP echoes → `(loss_percent, avg_ms)`. Parses the summary
/// lines ping always prints, so no locale fragility is introduced:
/// `"4 packets transmitted, 4 received, 0% packet loss"` and
/// `"rtt min/avg/max/mdev = 0.4/0.6/…"`.
fn ping_stats(host: &str) -> Option<(u32, f64)> {
    if !crate::backend::system::binary_exists("ping") {
        return None;
    }
    let out = Command::new("ping")
        .args(["-c", "4", "-W", "1", host])
        .output()
        .ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let loss = stdout.lines().find_map(|l| {
        l.split('%')
            .next()?
            .split(", ")
            .last()?
            .trim()
            .parse::<u32>()
            .ok()
    })?;
    let avg = stdout
        .lines()
        .find(|l| l.contains("min/avg/max"))
        .and_then(|l| l.split('=').nth(1))
        .and_then(|s| s.trim().split('/').nth(1))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    Some((loss, avg))
}
