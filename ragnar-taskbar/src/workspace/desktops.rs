// Workspace count is compositor policy, not protocol data — the
// ext-workspace `create_workspace` request exists but labwc does not
// advertise that capability, so the honest mechanism on the current
// session compositor is its rc.xml `<desktops number="N">` plus a
// live reconfigure (`labwc --reconfigure` sends SIGHUP to the
// running instance). This file is deliberately labwc-specific; a
// future compositor replaces it without touching the applet.
use std::path::PathBuf;

/// Locate the session compositor's rc.xml. The Ragnar session runs
/// labwc with `-C ~/.config/ragnar-desktop/labwc`; a stock labwc
/// session would use `~/.config/labwc`. Neither exists → None, and
// the settings UI disables the count control.
fn rc_path() -> Option<PathBuf> {
    let home = gtk4::glib::user_config_dir();
    [
        home.join("ragnar-desktop/labwc/rc.xml"),
        home.join("labwc/rc.xml"),
    ]
    .into_iter()
    .find(|cand| cand.is_file())
}

/// Whether a writable workspace-count config exists — the settings
/// page greys its control when the mechanism is unavailable.
pub fn available() -> bool {
    rc_path().is_some()
}

/// Current `<desktops number="N">` from the compositor config.
/// Returns None when the file or attribute is absent.
pub fn count() -> Option<u32> {
    let xml = std::fs::read_to_string(rc_path()?).ok()?;
    parse_count(&xml)
}

/// Extract the number attribute from `<desktops number="N">` —
/// pure parser so the XML handling stays unit-testable.
fn parse_count(xml: &str) -> Option<u32> {
    let tag = xml.find("<desktops")?;
    let rest = &xml[tag..];
    let num = rest.find("number=\"")? + "number=\"".len();
    let end = rest[num..].find('"')? + num;
    rest[num..end].parse().ok()
}

/// Rewrite `<desktops number="N">` inside an rc.xml document —
/// pure transformation, the file I/O lives in `set_count`.
fn rewrite_count(xml: &str, n: u32) -> Option<String> {
    let tag = xml.find("<desktops")?;
    let rest = &xml[tag..];
    let gt = rest.find('>')?;
    let head = &rest[..gt];
    let new_head = match head.find("number=\"") {
        Some(p) => {
            let start = p + "number=\"".len();
            let end = head[start..].find('"').map(|e| e + start).unwrap_or(start);
            format!("{}number=\"{}\"{}", &head[..p], n, &head[end + 1..])
        }
        None => format!("{head} number=\"{n}\""),
    };
    Some(format!("{}{}{}", &xml[..tag], new_head, &rest[gt..]))
}

/// Write a new `<desktops number="N">` and ask the running
/// compositor to reload. Returns false when the rc.xml has no
/// desktops element to rewrite — we never guess a location inside
/// foreign XML.
pub fn set_count(n: u32) -> bool {
    let Some(path) = rc_path() else { return false };
    let Ok(xml) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Some(out) = rewrite_count(&xml, n) else {
        return false;
    };
    if std::fs::write(&path, out).is_err() {
        return false;
    }
    // Live-reload: --reconfigure SIGHUPs the running compositor.
    // spawn_command_line_async reaps the child — Command::spawn
    // leaves a zombie per call.
    gtk4::glib::spawn_command_line_async("labwc --reconfigure").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_number() {
        let xml = "<labwc_config><desktops number=\"4\"><popupTime>1000</popupTime></desktops></labwc_config>";
        assert_eq!(parse_count(xml), Some(4));
    }

    #[test]
    fn parse_missing_returns_none() {
        assert_eq!(parse_count("<labwc_config><core/></labwc_config>"), None);
        assert_eq!(
            parse_count("<labwc_config><desktops></desktops></labwc_config>"),
            None
        );
    }

    #[test]
    fn rewrite_replaces_number() {
        let xml = "<labwc_config><desktops number=\"2\"><popupTime>1000</popupTime></desktops></labwc_config>";
        let out = rewrite_count(xml, 6).unwrap();
        assert!(out.contains("<desktops number=\"6\">"));
        assert!(out.contains("<popupTime>1000</popupTime>"));
        assert!(out.starts_with("<labwc_config>"));
    }

    #[test]
    fn rewrite_inserts_missing_attr() {
        let xml = "<labwc_config><desktops><popupTime>1000</popupTime></desktops></labwc_config>";
        let out = rewrite_count(xml, 3).unwrap();
        assert!(out.contains("<desktops number=\"3\">"));
    }

    #[test]
    fn rewrite_without_tag_fails() {
        assert_eq!(
            rewrite_count("<labwc_config><core/></labwc_config>", 2),
            None
        );
    }
}
