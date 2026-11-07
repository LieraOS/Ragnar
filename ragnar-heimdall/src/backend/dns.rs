//! DNS backend: reads systemd-resolved state via `resolvectl -j
//! status` (unprivileged) and mutates resolver configuration through
//! `pkexec` + `ragnar-heimdall-helper` — same trust model as ufw/routing.
//!
//! Global settings (DNSSEC, DNS-over-TLS, LLMNR, mDNS) and per-link
//! DNS servers are kept separate, matching resolved's own model.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;
use std::time::Instant;

/// Global resolved settings — the first object of `resolvectl -j
/// status` carries these fields without an `ifname`.
#[derive(Debug, Clone)]
pub struct DnsGlobal {
    /// `"yes"`, `"no"`, `"allow-downgrade"` or unsupported variant.
    pub dnssec: String,
    /// `"yes"`, `"no"` or `"opportunistic"`.
    pub dns_over_tls: String,
    /// `"yes"`/`"no"` — link-local multicast name resolution.
    pub llmnr: String,
    /// `"yes"`/`"no"` — multicast DNS (.local).
    pub mdns: String,
    /// resolv.conf operating mode (`stub`, `uplink`, ...).
    pub resolv_conf_mode: String,
    /// Whether the running resolved supports DNSSEC at all.
    pub dnssec_supported: bool,
}

/// DNS state of one link (`ifname` section of the status output).
#[derive(Debug, Clone)]
pub struct DnsLink {
    /// Interface name (`wlan0`, `enp2s0`, ...).
    pub ifname: String,
    /// Kernel interface index.
    pub ifindex: u32,
    /// Whether this link's servers are used for the default route.
    pub default_route: bool,
    /// Server currently answering queries, when resolved reports one.
    pub current_server: Option<String>,
    /// All configured servers in order.
    pub servers: Vec<String>,
    /// Search/routing domains (`~.` makes the link DNS-only for itself).
    pub domains: Vec<String>,
}

/// Complete snapshot: global settings plus every link.
#[derive(Debug, Clone)]
pub struct DnsStatus {
    pub global: DnsGlobal,
    pub links: Vec<DnsLink>,
}

/// Raw JSON shape of one status object — global objects lack
/// `ifname`, link objects always carry one.
#[derive(Debug, Deserialize)]
struct RawStatus {
    #[serde(default)]
    ifname: Option<String>,
    #[serde(default)]
    ifindex: Option<u32>,
    #[serde(default)]
    default_route: Option<bool>,
    #[serde(default, rename = "defaultRoute")]
    default_route_camel: Option<bool>,
    #[serde(default)]
    current_server: Option<RawServer>,
    #[serde(default, rename = "currentServer")]
    current_server_camel: Option<RawServer>,
    #[serde(default)]
    servers: Option<Vec<RawServer>>,
    #[serde(default)]
    domains: Option<Vec<RawDomain>>,
    #[serde(default)]
    dnssec: Option<String>,
    #[serde(default, rename = "dnsOverTLS")]
    dns_over_tls: Option<String>,
    #[serde(default)]
    llmnr: Option<String>,
    #[serde(default, rename = "mDNS")]
    mdns: Option<String>,
    #[serde(default, rename = "resolvConfMode")]
    resolv_conf_mode: Option<String>,
    #[serde(default, rename = "dnssecSupported")]
    dnssec_supported: Option<bool>,
}

/// `{"addressString": "1.2.3.4", ...}` shape for servers.
#[derive(Debug, Deserialize)]
struct RawServer {
    #[serde(default, rename = "addressString")]
    address_string: Option<String>,
}

/// `{"name": "example.com"}` domain entries, when present.
#[derive(Debug, Deserialize)]
struct RawDomain {
    #[serde(default)]
    name: Option<String>,
}

/// Whether resolved is managing DNS at all — hides the page when
/// the binary is missing (e.g. non-systemd systems).
pub fn available() -> bool {
    super::system::binary_exists("resolvectl")
}

/// Full resolved snapshot. Fails cleanly when resolved is not
/// running — the UI shows a status row instead of a dead page.
pub fn status() -> Result<DnsStatus> {
    let output = Command::new("resolvectl")
        .args(["-j", "status"])
        .output()
        .context("failed to run `resolvectl -j status`")?;
    anyhow::ensure!(
        output.status.success(),
        "resolvectl exited with {} — is systemd-resolved running?",
        output.status
    );
    let raw: Vec<RawStatus> =
        serde_json::from_slice(&output.stdout).context("failed to parse resolvectl JSON")?;

    let mut global = None;
    let mut links = Vec::new();
    for item in &raw {
        match &item.ifname {
            // Objects without an ifname are the global section.
            None => {
                global = Some(DnsGlobal {
                    dnssec: item.dnssec.clone().unwrap_or_else(|| "no".into()),
                    dns_over_tls: item.dns_over_tls.clone().unwrap_or_else(|| "no".into()),
                    llmnr: item.llmnr.clone().unwrap_or_else(|| "no".into()),
                    mdns: item.mdns.clone().unwrap_or_else(|| "no".into()),
                    resolv_conf_mode: item
                        .resolv_conf_mode
                        .clone()
                        .unwrap_or_else(|| "stub".into()),
                    dnssec_supported: item.dnssec_supported.unwrap_or(false),
                });
            }
            Some(ifname) => {
                links.push(DnsLink {
                    ifname: ifname.clone(),
                    ifindex: item.ifindex.unwrap_or(0),
                    default_route: item
                        .default_route
                        .or(item.default_route_camel)
                        .unwrap_or(false),
                    current_server: item
                        .current_server
                        .as_ref()
                        .or(item.current_server_camel.as_ref())
                        .and_then(|s| s.address_string.clone()),
                    servers: item
                        .servers
                        .iter()
                        .flatten()
                        .filter_map(|s| s.address_string.clone())
                        .collect(),
                    domains: item
                        .domains
                        .iter()
                        .flatten()
                        .filter_map(|d| d.name.clone())
                        .collect(),
                });
            }
        }
    }
    anyhow::ensure!(
        global.is_some() || !links.is_empty(),
        "resolvectl returned an empty status"
    );
    Ok(DnsStatus {
        global: global.unwrap_or(DnsGlobal {
            dnssec: "no".into(),
            dns_over_tls: "no".into(),
            llmnr: "no".into(),
            mdns: "no".into(),
            resolv_conf_mode: "stub".into(),
            dnssec_supported: false,
        }),
        links,
    })
}

/// Set the DNS servers of one link. Empty `servers` reverts to
/// whatever the link's configuration sources provide (DHCP).
pub fn set_servers(ifname: &str, servers: &[String]) -> Result<()> {
    if servers.is_empty() {
        return run(&["revert", ifname]).context("revert DNS servers");
    }
    let mut args: Vec<String> = vec!["dns".into(), ifname.into()];
    args.extend(servers.iter().map(|s| s.trim().to_string()));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("set DNS servers")
}

/// Set search/routing domains for a link (empty clears the list).
pub fn set_domains(ifname: &str, domains: &[String]) -> Result<()> {
    let mut args: Vec<String> = vec!["domain".into(), ifname.into()];
    if domains.is_empty() {
        args.push("~.".into());
    } else {
        args.extend(domains.iter().map(|d| d.trim().to_string()));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("set search domains")
}

/// Toggle a global resolved feature. `verb` is one of `dnssec`,
/// `dnsovertls`, `llmnr`, `mdns`; `value` the resolvectl argument
/// (`yes`, `no`, `allow-downgrade`, `opportunistic`).
pub fn set_feature(verb: &'static str, value: &str) -> Result<()> {
    run(&[verb, value]).with_context(|| format!("resolvectl {verb} {value}"))
}

/// Drop every cached answer — the standard "weird DNS" remedy.
pub fn flush_caches() -> Result<()> {
    run(&["flush-caches"]).context("flush DNS caches")
}

/// Live resolution test through resolved: returns `(addresses,
/// elapsed_ms)` for `name`. Runs unprivileged — `resolvectl query`
/// is a read operation like `getent`.
pub fn query(name: &str) -> Result<(Vec<String>, u128)> {
    let started = Instant::now();
    let output = Command::new("resolvectl")
        .args(["query", "--legend=no", name.trim()])
        .output()
        .context("failed to run `resolvectl query`")?;
    let elapsed = started.elapsed().as_millis();
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    // Lines look like "example.com: 93.184.216.34" — collect the
    // answer column, ignoring informational footer lines.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let addrs: Vec<String> = stdout
        .lines()
        .filter_map(|l| l.split(':').nth(1).map(str::trim))
        .flat_map(|s| s.split_whitespace().map(str::to_string))
        .collect();
    Ok((addrs, elapsed))
}

/// Execute `resolvectl` with `args` through pkexec + helper. The
/// helper whitelists the verb itself; this side only validates that
/// no empty value sneaks into the argument vector.
fn run(args: &[&str]) -> Result<()> {
    anyhow::ensure!(
        args.iter().all(|a| !a.trim().is_empty()),
        "empty argument rejected"
    );
    let prog = if std::path::Path::new(super::ufw::HELPER_PATH).is_file() {
        super::ufw::HELPER_PATH.to_string()
    } else {
        super::system::find_binary("resolvectl")
            .context("resolvectl binary not found")?
            .to_string_lossy()
            .into_owned()
    };
    let mut call = Command::new("pkexec");
    call.arg(&prog);
    if prog == super::ufw::HELPER_PATH {
        call.arg("resolvectl");
    }
    let output = call.args(args).output().context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("resolvectl failed ({}): {}", output.status, stderr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_global_and_links() {
        let json = r#"[
          {"dnssec":"no","dnsOverTLS":"no","llmnr":"yes","mDNS":"no",
           "resolvConfMode":"stub","dnssecSupported":false},
          {"ifname":"wlan0","ifindex":4,"defaultRoute":true,
           "currentServer":{"addressString":"192.168.1.1"},
           "servers":[{"addressString":"192.168.1.1"},
                      {"addressString":"fd00::1"}]}
        ]"#;
        let raw: Vec<RawStatus> = serde_json::from_str(json).unwrap();
        assert_eq!(raw.len(), 2);
        assert!(raw[0].ifname.is_none());
        assert_eq!(raw[1].ifname.as_deref(), Some("wlan0"));
        assert_eq!(raw[1].servers.as_ref().unwrap().len(), 2);
    }
}
