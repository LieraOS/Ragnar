// Copyright (C) Jörg Schröder
//! Search matching for the hub search field.
//!
//! Case-insensitive substring matching across title, subtitle and
//! keywords. Kept free of GTK so it is unit-testable.

use super::Entry;

/// Whether `entry` matches `query` (empty query matches everything).
pub fn matches(entry: &Entry, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    let mut haystacks = std::iter::once(entry.title)
        .chain(std::iter::once(entry.subtitle))
        .chain(entry.keywords.iter().copied())
        .chain(std::iter::once(entry.id));
    haystacks.any(|h| h.to_lowercase().contains(&needle))
}

/// All catalog entries matching `query`, preserving catalog order.
pub fn filter(entries: &[&'static Entry], query: &str) -> Vec<&'static Entry> {
    entries
        .iter()
        .copied()
        .filter(|e| matches(e, query))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Requirement;
    use crate::registry::Action;

    static DEMO: Entry = Entry {
        id: "sound",
        title: "Sound",
        subtitle: "Volume, devices and effects",
        icon: "audio-speakers-symbolic",
        category: "system",
        action: Action::Launch("lierasound"),
        requirement: Requirement::Always,
        keywords: &["audio", "volume", "microphone"],
    };

    #[test]
    fn empty_query_matches() {
        assert!(matches(&DEMO, ""));
        assert!(matches(&DEMO, "   "));
    }

    #[test]
    fn matches_title_case_insensitive() {
        assert!(matches(&DEMO, "sou"));
        assert!(matches(&DEMO, "SOUND"));
    }

    #[test]
    fn matches_keyword_and_id() {
        assert!(matches(&DEMO, "microphone"));
        assert!(matches(&DEMO, "sound"));
    }

    #[test]
    fn rejects_unrelated() {
        assert!(!matches(&DEMO, "printer"));
    }
}
