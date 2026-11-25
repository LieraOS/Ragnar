// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Report assembly: group entries by source, sort groups by error
// count, render the themed output with optional ANSI colours.

use crate::entry::Entry;
use std::collections::BTreeMap;

/// Render the full report as plain text or ANSI-coloured text.
/// Groups are sorted by error count (descending), then by name.
pub fn render(entries: &[Entry], filter: Option<&str>, colour: bool) -> String {
    let needle = filter.map(|f| f.to_lowercase());
    let mut groups: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        if let Some(n) = &needle {
            if !e.group.to_lowercase().contains(n.as_str()) {
                continue;
            }
        }
        groups.entry(e.group.clone()).or_default().push(e);
    }
    let mut sorted: Vec<_> = groups.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    let total: usize = sorted.iter().map(|(_, v)| v.len()).sum();
    let bold = |s: &str| {
        if colour {
            format!("\x1b[1;31m{s}\x1b[0m")
        } else {
            s.into()
        }
    };
    let dim = |s: &str| {
        if colour {
            format!("\x1b[2m{s}\x1b[0m")
        } else {
            s.into()
        }
    };

    let mut out = String::new();
    out.push_str(&bold("RAGNAROK — system errors\n\n"));
    if total == 0 {
        out.push_str("Ragnarök abgewendet — the system still stands. No errors recorded.\n");
        return out;
    }
    for (group, items) in &sorted {
        out.push_str(&bold(&format!("── {} ({}) ──\n", group, items.len())));
        for e in items {
            if e.timestamp.is_empty() {
                out.push_str(&format!("    {}\n", e.message));
            } else {
                out.push_str(&format!("    {}  {}\n", dim(&e.timestamp), e.message));
            }
        }
        out.push('\n');
    }
    out.push_str(&dim(&format!(
        "{total} errors in {} groups\n",
        sorted.len()
    )));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(group: &str, msg: &str) -> Entry {
        Entry {
            group: group.into(),
            timestamp: "t".into(),
            message: msg.into(),
        }
    }

    #[test]
    fn groups_sorted_by_count() {
        let out = render(&[e("a", "x"), e("b", "x"), e("b", "y")], None, false);
        assert!(out.find("── b (2)").unwrap() < out.find("── a (1)").unwrap());
    }

    #[test]
    fn filter_is_case_insensitive() {
        let out = render(
            &[e("lightdm.service", "x"), e("kernel", "y")],
            Some("LightDM"),
            false,
        );
        assert!(out.contains("lightdm.service"));
        assert!(!out.contains("kernel (1)"));
    }

    #[test]
    fn empty_shows_the_easter_egg_line() {
        assert!(render(&[], None, false).contains("system still stands"));
    }
}
