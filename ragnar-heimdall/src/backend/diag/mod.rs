//! Network diagnostics: a battery of read-only checks grouped into
//! selectable suites — the user picks what to run. Suites answer
//! "why is the network broken" — link state, addressing, routing,
//! DNS, services, radio, internet performance. Everything here is
//! unprivileged; no check needs root or mutates state.

use std::collections::BTreeSet;

mod quick;

/// Outcome severity of one diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Warn,
    Fail,
    /// Check could not run (missing tool, no wifi interface, …).
    Skip,
}

/// One finished check: grouping, headline and detail line.
#[derive(Debug, Clone)]
pub struct Check {
    pub group: &'static str,
    pub name: String,
    pub status: Status,
    pub detail: String,
}

impl Check {
    pub(crate) fn new(
        group: &'static str,
        name: &str,
        status: Status,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            group,
            name: name.to_string(),
            status,
            detail: detail.into(),
        }
    }
}

/// One selectable diagnostic suite shown as a switch in the UI.
pub struct Suite {
    pub id: &'static str,
    pub tier: Tier,
    pub title: &'static str,
    pub description: &'static str,
}

/// Whether a suite belongs to the quick battery (normal users)
/// or the expert battery (power users).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Quick,
    Expert,
}

/// Every suite, in sidebar order of the UI's switch list.
/// `tier` splits them into the two UI groups — the first six are
/// the quick battery an everyday user runs with one click.
pub const SUITES: &[Suite] = &[
    Suite {
        id: "links",
        tier: Tier::Quick,
        title: "Links and addresses",
        description: "Interface states and all assigned IP addresses",
    },
    Suite {
        id: "routing",
        tier: Tier::Quick,
        title: "Routing and gateway",
        description: "Default route and gateway reachability",
    },
    Suite {
        id: "dns",
        tier: Tier::Quick,
        title: "DNS",
        description: "Configured nameservers and live resolution test",
    },
    Suite {
        id: "services",
        tier: Tier::Quick,
        title: "Services",
        description: "networkd, iwd and ufw unit states",
    },
    Suite {
        id: "wifi",
        tier: Tier::Quick,
        title: "Wi-Fi",
        description: "Radio state and current association",
    },
    Suite {
        id: "internet",
        tier: Tier::Quick,
        title: "Internet and speed",
        description: "External IP, latency, download and upload throughput",
    },
    Suite {
        id: "portal",
        tier: Tier::Expert,
        title: "Captive portal",
        description: "Detect hotel/hotspot login pages intercepting traffic",
    },
    Suite {
        id: "ipv6",
        tier: Tier::Expert,
        title: "IPv6 connectivity",
        description: "v6 default route and external reachability",
    },
    Suite {
        id: "path",
        tier: Tier::Expert,
        title: "Path analysis and MTU",
        description: "tracepath hops and path-MTU probing",
    },
    Suite {
        id: "timesync",
        tier: Tier::Expert,
        title: "Time synchronisation",
        description: "NTP state — a skewed clock breaks TLS",
    },
    Suite {
        id: "neighbors",
        tier: Tier::Expert,
        title: "Neighbor table",
        description: "ARP/NDP cache health — L2 trouble on the LAN",
    },
    Suite {
        id: "listeners",
        tier: Tier::Expert,
        title: "Listening sockets",
        description: "Ports this machine exposes to the network",
    },
    Suite {
        id: "vpnstate",
        tier: Tier::Expert,
        title: "VPN state",
        description: "WireGuard peers and tun/tap devices",
    },
    Suite {
        id: "dnsdeep",
        tier: Tier::Expert,
        title: "DNS depth",
        description: "Resolver posture and query latency",
    },
    Suite {
        id: "wifideep",
        tier: Tier::Expert,
        title: "Wi-Fi depth",
        description: "BSSID, channel, bitrate and signal via iw",
    },
];

/// Suite ids belonging to the quick battery.
pub const QUICK_SUITES: &[&str] = &["links", "routing", "dns", "services", "wifi", "internet"];

/// Facts shared by several suites — gathered once per run.
struct Context {
    ifaces: Vec<super::model::NetworkInterface>,
    route: Option<String>,
    gateway_reachable: bool,
}

/// Run the suites whose ids are in `enabled`, in SUITES order.
pub fn run(enabled: &BTreeSet<&'static str>) -> Vec<Check> {
    let ctx = Context {
        ifaces: super::collector::collect_interfaces().unwrap_or_default(),
        route: super::system::default_route().ok().flatten(),
        gateway_reachable: false,
    };
    let mut out = Vec::new();
    // Gather gateway reachability eagerly — dns and internet gate on
    // it regardless of whether the routing suite itself is enabled.
    let mut ctx = ctx;
    if let Some(gw) = ctx
        .route
        .as_deref()
        .and_then(|r| r.split_whitespace().next().map(str::to_string))
    {
        ctx.gateway_reachable = quick::ping(&gw);
    }

    for suite in SUITES {
        if !enabled.contains(suite.id) {
            continue;
        }
        out.extend(match suite.id {
            "links" => quick::suite_links(&ctx),
            "routing" => quick::suite_routing(&ctx),
            "dns" => quick::suite_dns(&ctx),
            "services" => quick::suite_services(),
            "wifi" => quick::suite_wifi(&ctx),
            "internet" => quick::suite_internet(&ctx),
            "portal" => super::diag_expert::suite_portal(),
            "ipv6" => super::diag_expert::suite_ipv6(),
            "path" => super::diag_expert::suite_path(),
            "timesync" => super::diag_expert::suite_timesync(),
            "neighbors" => super::diag_deep::suite_neighbors(),
            "listeners" => super::diag_deep::suite_listeners(),
            "vpnstate" => super::diag_deep::suite_vpnstate(),
            "dnsdeep" => super::diag_deep::suite_dns_deep(),
            "wifideep" => super::diag_deep::suite_wifi_deep(&ctx.ifaces),
            _ => Vec::new(),
        });
    }
    out
}

/// Serialise a finished battery into a markdown report the user can
/// save or attach to a bug report.
pub fn report_md(checks: &[Check]) -> String {
    let mut pass = 0;
    let mut warn = 0;
    let mut fail = 0;
    let mut skip = 0;
    for c in checks {
        match c.status {
            Status::Pass => pass += 1,
            Status::Warn => warn += 1,
            Status::Fail => fail += 1,
            Status::Skip => skip += 1,
        }
    }
    let mut md = String::from("# Heimdall network diagnosis\n\n");
    md.push_str(&format!(
        "**{} passed · {} warnings · {} failed · {} skipped**\n\n",
        pass, warn, fail, skip
    ));
    let mut last_group = "";
    for c in checks {
        if c.group != last_group {
            md.push_str(&format!(
                "## {}\n\n| Check | Status | Detail |\n|---|---|---|\n",
                c.group
            ));
            last_group = c.group;
        }
        let status = match c.status {
            Status::Pass => "pass",
            Status::Warn => "warn",
            Status::Fail => "fail",
            Status::Skip => "skip",
        };
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            c.name.replace('|', "\\|"),
            status,
            c.detail.replace('|', "\\|")
        ));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suites_have_unique_ids() {
        let mut ids = BTreeSet::new();
        for suite in SUITES {
            assert!(ids.insert(suite.id), "duplicate suite id {}", suite.id);
        }
    }

    #[test]
    fn disabled_suites_produce_no_checks() {
        let out = run(&BTreeSet::new());
        assert!(out.is_empty());
    }
}
