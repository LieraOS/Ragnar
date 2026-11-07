//! UFW firewall backend. Every `ufw` call needs root, so commands go
//! through `pkexec` and a tiny whitelisting helper shipped by the
//! package — its polkit action (`org.lieraos.ragnar.Heimdall.admin`) is
//! `auth_admin_keep`, so the user authenticates once per batch of
//! operations instead of once per click. Reading state needs root
//! too (`ufw status` refuses unprivileged users), so even the
//! read-only path uses the helper.

use anyhow::{Context, Result};
use std::process::Command;

/// Privileged helper shipped as `/usr/lib/ragnar-heimdall/ragnar-heimdall-helper`
/// by the .deb. When the package is not installed (development run)
/// we fall back to `pkexec ufw` directly — same effect, but the
/// generic pkexec action prompts on every call.
///
/// `pub(crate)`: the routing backend shares the helper for its
/// whitelisted `ip route` verb chain.
pub(crate) const HELPER_PATH: &str = "/usr/lib/ragnar-heimdall/ragnar-heimdall-helper";

/// One parsed rule line from `ufw status numbered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UfwRule {
    /// Number shown by `ufw status numbered` — needed for delete.
    pub number: u32,
    /// Target column, e.g. `22/tcp` or `80` or `Anywhere`.
    pub to: String,
    /// Action column, e.g. `ALLOW IN` or `DENY IN`.
    pub action: String,
    /// Source column, e.g. `Anywhere`.
    pub from: String,
    /// Rule applies to IPv6 only (`(v6)` marker).
    pub ipv6: bool,
}

/// Policy applied to a direction when no rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Allow,
    Deny,
    Reject,
    Disabled,
}

/// Parsed `ufw status verbose` snapshot.
#[derive(Debug, Clone)]
pub struct UfwStatus {
    pub enabled: bool,
    pub incoming: Policy,
    pub outgoing: Policy,
    pub rules: Vec<UfwRule>,
}

/// Whether the `ufw` binary exists at all — the UI hides the whole
/// firewall page when it does not. `ufw` lives in sbin, which user
/// PATHs often lack, so the standard system dirs are probed too.
pub fn available() -> bool {
    super::system::binary_exists("ufw")
}

/// Read the current status. Fails with a clean error when `ufw`
/// is missing or the user cancels the polkit prompt.
pub fn status() -> Result<UfwStatus> {
    let verbose = run(&["status", "verbose"]).context("ufw status verbose")?;
    let numbered = run(&["status", "numbered"]).context("ufw status numbered")?;
    Ok(UfwStatus {
        enabled: parse_enabled(&verbose),
        incoming: parse_policy(&verbose, "incoming"),
        outgoing: parse_policy(&verbose, "outgoing"),
        rules: parse_rules(&numbered),
    })
}

/// Turn the firewall on or off.
pub fn set_enabled(on: bool) -> Result<()> {
    let args: &[&str] = if on {
        &["--force", "enable"]
    } else {
        &["disable"]
    };
    run(args).context("toggle firewall")?;
    Ok(())
}

/// Add an incoming rule. `port` may be a number, service name or
/// `start:end` range; `proto` is `tcp`, `udp` or empty for both.
/// `from` restricts the source (empty = Anywhere).
pub fn add_rule(allow: bool, port: &str, proto: &str, from: &str) -> Result<()> {
    let mut args: Vec<String> = vec![if allow { "allow" } else { "deny" }.into()];
    if from.trim().is_empty() {
        let mut to = port.trim().to_string();
        if !proto.is_empty() && proto != "any" {
            to.push('/');
            to.push_str(proto);
        }
        args.push(to);
    } else {
        // `ufw allow from <addr> to any port <p> proto <tcp>`
        args.extend([
            "from".into(),
            from.trim().to_string(),
            "to".into(),
            "any".into(),
        ]);
        args.push("port".into());
        args.push(port.trim().to_string());
        if !proto.is_empty() && proto != "any" {
            args.push("proto".into());
            args.push(proto.to_string());
        }
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("add firewall rule")?;
    Ok(())
}

/// The full ufw rule surface for the "Erweitert" section — every
/// field maps 1:1 onto a `ufw` argument; empty means "omit".
#[derive(Debug, Clone, Default)]
pub struct AdvRule {
    /// allow | deny | reject | limit
    pub action: String,
    /// "in" | "out" — empty means ufw's default (in).
    pub direction: String,
    /// Interface name (eth0, wlan0) — empty = all.
    pub iface: String,
    /// `log` flag for the rule.
    pub log: bool,
    /// Source address/subnet — empty = Anywhere.
    pub from: String,
    /// Destination address — empty = Anywhere.
    pub to_addr: String,
    /// Port/service — empty = any port.
    pub port: String,
    /// tcp | udp | any
    pub proto: String,
    /// Free-text comment (ufw `comment 'x'`).
    pub comment: String,
    /// Optional insert position (1-based) — None = append.
    pub position: Option<u32>,
}

/// Build the `ufw` argument vector for an advanced rule.
fn adv_args(s: &AdvRule) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    if let Some(n) = s.position {
        a.extend(["insert".into(), n.to_string()]);
    }
    a.push(if s.action.is_empty() {
        "allow".into()
    } else {
        s.action.clone()
    });
    if !s.direction.is_empty() {
        a.push(s.direction.clone());
    }
    if !s.iface.is_empty() {
        a.extend(["on".into(), s.iface.clone()]);
    }
    if s.log {
        a.push("log".into());
    }
    if !s.from.trim().is_empty() {
        a.extend(["from".into(), s.from.trim().to_string()]);
    }
    a.extend([
        "to".into(),
        if s.to_addr.trim().is_empty() {
            "any".into()
        } else {
            s.to_addr.trim().to_string()
        },
    ]);
    if !s.port.trim().is_empty() {
        a.extend(["port".into(), s.port.trim().to_string()]);
    }
    if !s.proto.is_empty() && s.proto != "any" {
        a.extend(["proto".into(), s.proto.clone()]);
    }
    if !s.comment.trim().is_empty() {
        a.extend(["comment".into(), s.comment.trim().to_string()]);
    }
    a
}

/// Add (or insert) an advanced rule.
pub fn add_rule_advanced(s: &AdvRule) -> Result<()> {
    let args = adv_args(s);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs).context("add advanced firewall rule")?;
    Ok(())
}

/// Reconstruct a minimal rule spec from a parsed status row — used
/// for move/reorder, which ufw can only express as delete + insert.
pub fn spec_from_parsed_pub(r: &UfwRule) -> AdvRule {
    spec_from_parsed(r)
}

fn spec_from_parsed(r: &UfwRule) -> AdvRule {
    let base = r
        .action
        .split_whitespace()
        .next()
        .unwrap_or("ALLOW")
        .to_lowercase();
    let direction = if r.action.contains("OUT") {
        "out"
    } else {
        "in"
    }
    .to_string();
    // "22/tcp" → port + proto; anything else stays the bare target.
    let (port, proto) =
        r.to.split_once('/')
            .map(|(p, pr)| (p.to_string(), pr.to_string()))
            .unwrap_or_else(|| {
                if r.to.chars().all(|c| c.is_ascii_digit() || c == ':') {
                    (r.to.clone(), "any".into())
                } else {
                    (String::new(), "any".into())
                }
            });
    AdvRule {
        action: base,
        direction,
        from: if r.from == "Anywhere" {
            String::new()
        } else {
            r.from.clone()
        },
        port,
        proto,
        position: None,
        ..Default::default()
    }
}

/// Move rule `number` to `new_pos` (1-based): delete it and insert
/// the reconstructed spec at the target slot. ufw has no native
/// move verb — this is the documented equivalent.
pub fn move_rule(number: u32, new_pos: u32, current: &UfwStatus) -> Result<()> {
    let Some(rule) = current.rules.iter().find(|r| r.number == number) else {
        anyhow::bail!("rule {number} not found");
    };
    let mut spec = spec_from_parsed(rule);
    spec.position = Some(new_pos);
    delete_rule(number)?;
    add_rule_advanced(&spec)
}

/// Delete the rule carrying `number` in `ufw status numbered`.
pub fn delete_rule(number: u32) -> Result<()> {
    run(&["--force", "delete", &number.to_string()]).context("delete firewall rule")?;
    Ok(())
}

/// Set the default policy for a direction (`in`/`out`).
pub fn set_default(direction: &str, allow: bool) -> Result<()> {
    run(&["default", if allow { "allow" } else { "deny" }, direction])
        .context("set default policy")?;
    Ok(())
}

/// Execute `ufw` with `args` through pkexec + helper and return
/// stdout. stderr is included in the error message.
fn run(args: &[&str]) -> Result<String> {
    // Prefer the packaged helper (single polkit prompt via
    // auth_admin_keep); fall back to pkexec ufw in development where
    // the .deb is not installed. Absolute paths everywhere — pkexec
    // runs with a minimal environment.
    let prog = if std::path::Path::new(HELPER_PATH).is_file() {
        HELPER_PATH.to_string()
    } else {
        super::system::find_binary("ufw")
            .context("ufw binary not found")?
            .to_string_lossy()
            .into_owned()
    };
    let mut call = Command::new("pkexec");
    call.arg(&prog);
    // The helper dispatches on the tool name; the pkexec fallback
    // targets `ufw` directly and takes plain arguments.
    if prog == HELPER_PATH {
        call.arg("ufw");
    }
    let output = call.args(args).output().context("failed to run pkexec")?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!("ufw failed ({}): {}", output.status, stderr)
}

/// `Status: active`/`Status: inactive` from the verbose output.
fn parse_enabled(verbose: &str) -> bool {
    verbose
        .lines()
        .find_map(|l| l.trim().strip_prefix("Status:"))
        .map(|v| v.trim() == "active")
        .unwrap_or(false)
}

/// `Default: deny (incoming), allow (outgoing), disabled (routed)`.
fn parse_policy(verbose: &str, direction: &str) -> Policy {
    let default = verbose
        .lines()
        .find_map(|l| l.trim().strip_prefix("Default:"))
        .unwrap_or_default();
    let needle = format!("({direction})");
    for part in default.split(',') {
        let part = part.trim();
        if part.ends_with(&needle) {
            return match part.split_whitespace().next() {
                Some("allow") => Policy::Allow,
                Some("deny") => Policy::Deny,
                Some("reject") => Policy::Reject,
                Some("disabled") => Policy::Disabled,
                _ => Policy::Disabled,
            };
        }
    }
    Policy::Disabled
}

/// Parse the fixed-width rule table of `ufw status numbered`.
/// Columns are located via the `To`/`Action`/`From` header offsets,
/// which keeps multi-word fields ("ALLOW IN", "Anywhere (v6)") intact.
fn parse_rules(numbered: &str) -> Vec<UfwRule> {
    let mut rules = Vec::new();
    let mut cols: Option<(usize, usize)> = None; // (action_start, from_start)
    for line in numbered.lines() {
        let t = line.trim_start();
        if t.starts_with("To") {
            cols = Some((
                line.find("Action").unwrap_or(0),
                line.find("From").unwrap_or(0),
            ));
            continue;
        }
        let Some((action_at, from_at)) = cols else {
            continue;
        };
        if !t.starts_with('[') {
            continue;
        }
        let Some(num) = t
            .trim_start_matches('[')
            .split(']')
            .next()
            .and_then(|n| n.trim().parse::<u32>().ok())
        else {
            continue;
        };
        // Column slicing needs byte offsets measured on the raw line.
        let field = |a: usize, b: usize| -> String {
            line.get(a..b).unwrap_or_default().trim().to_string()
        };
        let to = {
            // Everything between the `]` and the Action column.
            let start = line.find(']').map(|i| i + 1).unwrap_or(0);
            field(start, action_at)
        };
        let action = field(action_at, from_at);
        let mut from = field(from_at, line.len());
        let ipv6 = from.contains("(v6)") || to.contains("(v6)");
        from = from.replace("(v6)", "").trim().to_string();
        let to = to.replace("(v6)", "").trim().to_string();
        if !to.is_empty() {
            rules.push(UfwRule {
                number: num,
                to,
                action,
                from,
                ipv6,
            });
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERBOSE: &str = "Status: active\n\
        Logging: on (low)\n\
        Default: deny (incoming), allow (outgoing), disabled (routed)\n\
        New profiles: skip\n";

    // Exact real-world `ufw status numbered` layout: the header is
    // indented five spaces so its To/Action/From columns line up with
    // the `[ N]` prefixed rows that start at column 0.
    const NUMBERED: &str = concat!(
        "Status: active\n",
        "\n",
        "     To                         Action      From\n",
        "     --                         ------      ----\n",
        "[ 1] 22/tcp                     ALLOW IN    Anywhere\n",
        "[ 2] 443/tcp                    DENY IN     Anywhere (v6)\n",
    );

    #[test]
    fn enabled_parse() {
        assert!(parse_enabled(VERBOSE));
        assert!(!parse_enabled("Status: inactive\n"));
    }

    #[test]
    fn policy_parse() {
        assert_eq!(parse_policy(VERBOSE, "incoming"), Policy::Deny);
        assert_eq!(parse_policy(VERBOSE, "outgoing"), Policy::Allow);
    }

    #[test]
    fn rules_parse() {
        let rules = parse_rules(NUMBERED);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].number, 1);
        assert_eq!(rules[0].to, "22/tcp");
        assert_eq!(rules[0].action, "ALLOW IN");
        assert_eq!(rules[0].from, "Anywhere");
        assert!(!rules[0].ipv6);
        assert_eq!(rules[1].to, "443/tcp");
        assert_eq!(rules[1].action, "DENY IN");
        assert!(rules[1].ipv6);
    }
}
