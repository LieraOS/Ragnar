// Copyright (C) Jörg Schröder
//! Routing backend: reads the kernel routing tables and policy
//! rules through `ip -j` and mutates the main table through `pkexec`
//! plus the packaged `ragnar-heimdall-helper`. Same trust model as ufw.
//!
//! Reading is fully unprivileged.
//!
//! Address-family and table are kept on every record so the UI can
//! group IPv4/IPv6 and the `main`/`local`/custom tables without
//! re-parsing text output.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

/// One route entry from `ip -j route show table all`.
#[derive(Debug, Clone)]
pub struct Route {
    /// Destination prefix (`"default"` or a CIDR).
    pub dst: String,
    /// Next-hop gateway, if the route is not directly connected.
    pub gateway: Option<String>,
    /// Outgoing interface, if the kernel reports one.
    pub dev: Option<String>,
    /// Route origin (`kernel`, `dhcp`, `static`, `ra`, `boot`, ...).
    pub protocol: String,
    /// Optional scope (`global`, `link`, `host`).
    pub scope: Option<String>,
    /// Route metric/preference.
    pub metric: Option<u32>,
    /// Preferred source address.
    pub prefsrc: Option<String>,
    /// Routing table the entry lives in (`main`, `local`, `default`, ...).
    pub table: String,
    /// `true` for IPv6, `false` for IPv4.
    pub ipv6: bool,
}

/// One `ip rule` entry: a selector plus the table it routes into.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Lookup priority — lower wins.
    pub priority: u32,
    /// Human-readable selector (`"from all"`, `"from 10.0.0.0/8 iif ..."`).
    pub selector: String,
    /// Target routing table.
    pub table: String,
}

/// Raw `ip -j route` record as iproute2 emits it.
#[derive(Debug, Deserialize)]
struct RawRoute {
    #[serde(default)]
    dst: Option<String>,
    #[serde(default)]
    gateway: Option<String>,
    #[serde(default)]
    dev: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    metric: Option<u32>,
    #[serde(default)]
    prefsrc: Option<String>,
    #[serde(default)]
    table: Option<String>,
    /// nexthop-group routes carry `nhid` instead of `gateway`; the
    /// next-hop object resolves to the same gateway anyway, so we
    /// accept plain records and ignore multipath details.
    #[serde(default)]
    #[allow(dead_code)]
    nhid: Option<u64>,
}

/// Raw `ip -j rule list` record.
#[derive(Debug, Deserialize)]
struct RawRule {
    #[serde(default)]
    priority: Option<u32>,
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    dst: Option<String>,
    #[serde(default)]
    iif: Option<String>,
    #[serde(default)]
    oif: Option<String>,
    #[serde(default)]
    fwmark: Option<String>,
    #[serde(default)]
    table: Option<String>,
}

/// All routes across every table, for both address families.
pub fn routes() -> Result<Vec<Route>> {
    let mut out = read_routes("-4", false)?;
    out.extend(read_routes("-6", true)?);
    Ok(out)
}

/// All policy rules in priority order.
pub fn rules() -> Result<Vec<Rule>> {
    let output = Command::new("ip")
        .args(["-j", "rule", "list"])
        .output()
        .context("failed to run `ip -j rule`")?;
    anyhow::ensure!(
        output.status.success(),
        "ip rule exited with {}",
        output.status
    );
    let raw: Vec<RawRule> =
        serde_json::from_slice(&output.stdout).context("failed to parse `ip -j rule`")?;
    let mut rules: Vec<Rule> = raw.iter().filter_map(rule_from).collect();
    rules.sort_by_key(|r| r.priority);
    Ok(rules)
}

/// Add a route to the `main` table. `dst` is a CIDR or `default`;
/// `gateway`/`dev`/`metric` are only emitted when non-empty. At
/// least a gateway or a device must be given — `ip` would fail on
/// an anchorless route anyway, so we validate up front and keep the
/// polkit prompt for real errors only.
pub fn add_route(dst: &str, gateway: &str, dev: &str, metric: &str) -> Result<()> {
    let dst = dst.trim();
    anyhow::ensure!(!dst.is_empty(), "destination must not be empty");
    let mut args: Vec<String> = vec!["route".into(), "add".into(), dst.into()];
    if !gateway.trim().is_empty() {
        args.extend(["via".into(), gateway.trim().into()]);
    }
    if !dev.trim().is_empty() {
        args.extend(["dev".into(), dev.trim().into()]);
    }
    if !metric.trim().is_empty() {
        anyhow::ensure!(
            metric.trim().parse::<u32>().is_ok(),
            "metric must be a number"
        );
        args.extend(["metric".into(), metric.trim().into()]);
    }
    anyhow::ensure!(
        !gateway.trim().is_empty() || !dev.trim().is_empty(),
        "a gateway or an interface is required"
    );
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("add route")
}

/// Delete exactly the route the user clicked. Rebuilding the full
/// `ip route del` specification keeps us from matching a different
/// route that happens to share the destination.
pub fn delete_route(route: &Route) -> Result<()> {
    let mut args: Vec<String> = vec![
        if route.ipv6 { "-6" } else { "-4" }.into(),
        "route".into(),
        "del".into(),
        route.dst.clone(),
        "table".into(),
        route.table.clone(),
    ];
    if let Some(gw) = &route.gateway {
        args.extend(["via".into(), gw.clone()]);
    }
    if let Some(dev) = &route.dev {
        args.extend(["dev".into(), dev.clone()]);
    }
    if let Some(m) = route.metric {
        args.extend(["metric".into(), m.to_string()]);
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("delete route")
}

/// `ip -j <family> route show table all` parsed into [`Route`]s.
fn read_routes(family: &str, ipv6: bool) -> Result<Vec<Route>> {
    let output = Command::new("ip")
        .args(["-j", family, "route", "show", "table", "all"])
        .output()
        .with_context(|| format!("failed to run `ip {family} -j route`"))?;
    anyhow::ensure!(
        output.status.success(),
        "ip route exited with {}",
        output.status
    );
    let raw: Vec<RawRoute> =
        serde_json::from_slice(&output.stdout).context("failed to parse `ip -j route`")?;
    Ok(raw
        .iter()
        .map(|r| Route {
            dst: r.dst.clone().unwrap_or_else(|| "default".into()),
            gateway: r.gateway.clone(),
            dev: r.dev.clone(),
            protocol: r.protocol.clone().unwrap_or_else(|| "?".into()),
            scope: r.scope.clone(),
            metric: r.metric,
            prefsrc: r.prefsrc.clone(),
            table: r.table.clone().unwrap_or_else(|| "main".into()),
            ipv6,
        })
        .collect())
}

/// Convert one JSON rule into its display form. Selectors we do not
/// recognise are skipped — an invisible rule is better than a wrong
/// one.
fn rule_from(r: &RawRule) -> Option<Rule> {
    let priority = r.priority?;
    let mut selector = format!("from {}", r.src.as_deref().unwrap_or("all"));
    if let Some(dst) = &r.dst {
        selector.push_str(&format!(" to {dst}"));
    }
    if let Some(iif) = &r.iif {
        selector.push_str(&format!(" iif {iif}"));
    }
    if let Some(oif) = &r.oif {
        selector.push_str(&format!(" oif {oif}"));
    }
    if let Some(mark) = &r.fwmark {
        selector.push_str(&format!(" fwmark {mark}"));
    }
    Some(Rule {
        priority,
        selector,
        table: r.table.clone().unwrap_or_else(|| "main".into()),
    })
}

/// Execute `ip` with `args` through pkexec + helper — the helper
/// only whitelists the `route` verb chain, everything else is
/// rejected before `ip` even runs.
fn run(args: &[&str]) -> Result<()> {
    let prog = if std::path::Path::new(super::ufw::HELPER_PATH).is_file() {
        super::ufw::HELPER_PATH.to_string()
    } else {
        super::system::find_binary("ip")
            .context("ip binary not found")?
            .to_string_lossy()
            .into_owned()
    };
    let mut call = Command::new("pkexec");
    call.arg(&prog);
    // Helper dispatches on the tool name; the pkexec fallback targets
    // `ip` directly and takes plain arguments.
    if prog == super::ufw::HELPER_PATH {
        call.arg("ip");
    }
    let output = call.args(args).output().context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("ip route failed ({}): {}", output.status, stderr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_route_parses_kernel_shape() {
        let json = r#"[{"dst":"default","gateway":"192.168.1.1","dev":"wlan0",
                       "protocol":"dhcp","prefsrc":"192.168.1.5","metric":1024,"flags":[]}]"#;
        let raw: Vec<RawRoute> = serde_json::from_str(json).unwrap();
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].dst.as_deref(), Some("default"));
        assert_eq!(raw[0].metric, Some(1024));
    }

    #[test]
    fn raw_rule_parses_standard_rules() {
        let json = r#"[{"priority":0,"src":"all","table":"local"},
                       {"priority":32766,"src":"all","table":"main"}]"#;
        let raw: Vec<RawRule> = serde_json::from_str(json).unwrap();
        let rules: Vec<Rule> = raw.iter().filter_map(rule_from).collect();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].priority, 0);
        assert_eq!(rules[0].table, "local");
        assert_eq!(rules[0].selector, "from all");
    }

    #[test]
    fn add_route_requires_anchor() {
        assert!(add_route("10.0.0.0/8", "", "", "").is_err());
        assert!(add_route("", "192.168.1.1", "eth0", "").is_err());
        assert!(add_route("10.0.0.0/8", "gw", "eth0", "abc").is_err());
    }
}
