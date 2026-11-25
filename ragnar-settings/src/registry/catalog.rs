// Copyright (C) Jörg Schröder
//! The concrete settings catalog.
//!
//! Category order defines display order on the hub. Entries reference
//! category ids; each entry names its embedded panel id or the external
//! program to launch.

use super::Entry;
use crate::i18n::msgid;

/// One category shown as a group header on the hub.
#[derive(Debug, Clone, Copy)]
pub struct Category {
    /// Stable machine id matched by `Entry::category`.
    pub id: &'static str,
    /// English source string, translated at display time.
    pub title: &'static str,
}

/// Category groups in display order.
static CATEGORIES: &[Category] = &[
    Category {
        id: "system",
        title: msgid("System"),
    },
    Category {
        id: "personalization",
        title: msgid("Personalization"),
    },
    Category {
        id: "network",
        title: msgid("Network and Internet"),
    },
    Category {
        id: "programs",
        title: msgid("Programs"),
    },
    Category {
        id: "devices",
        title: msgid("Devices"),
    },
    Category {
        id: "accounts",
        title: msgid("Accounts and Security"),
    },
];

/// Category groups in display order.
pub fn categories() -> &'static [Category] {
    CATEGORIES
}

/// All catalog entries, in catalog order.
fn catalog() -> &'static [Entry] {
    super::entries::CATALOG
}

/// Tile ids that only appear in advanced mode. Whole areas too
/// deep for the normal view register here — deep rows inside an
/// otherwise normal panel guard themselves with
/// `crate::advanced::enabled()` instead and do not belong in this
/// list. Today: the compositor panel — deep rc.xml configuration
/// that would otherwise mean editing files by hand.
pub const ADVANCED_TILES: &[&str] = &["compositor", "windowrules", "autostart"];

/// Entries belonging to `category`, in catalog order.
pub fn entries_in(category: &str) -> Vec<&'static Entry> {
    catalog()
        .iter()
        .filter(|e| e.category == category)
        .collect()
}

/// All entries regardless of category (used by search).
pub fn all_entries() -> &'static [Entry] {
    catalog()
}
