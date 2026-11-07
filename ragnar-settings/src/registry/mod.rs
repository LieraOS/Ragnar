//! Entry registry: the static catalog of settings areas.
//!
//! [`Entry`] describes one tile on the hub: display name, icon,
//! category and what happens on activation. [`catalog`] defines the
//! concrete entries; [`search`] filters them for the search field.

mod catalog;
pub mod search;

pub use catalog::{all_entries, categories, entries_in, Category};

use crate::backend::Requirement;

/// How a tile behaves on activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open the embedded panel identified by this id.
    Panel(&'static str),
    /// Launch an external program (desktop id or binary name).
    Launch(&'static str),
}

/// One settings area shown as a hub tile.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Stable machine id, also used in logs and tests.
    pub id: &'static str,
    /// English source string, passed through gettext at display time.
    pub title: &'static str,
    /// Subtitle / description shown under the title.
    pub subtitle: &'static str,
    /// Freedesktop icon name.
    pub icon: &'static str,
    /// Category id from [`categories`].
    pub category: &'static str,
    /// Behavior on activation.
    pub action: Action,
    /// What must exist on the host for the tile to be shown.
    pub requirement: Requirement,
    /// Extra search keywords (not displayed).
    pub keywords: &'static [&'static str],
}
