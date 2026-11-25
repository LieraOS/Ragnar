// Copyright (C) Jörg Schröder
//! User preferences for the icon context menu, read from the
//! `org.lieraos.ragnar.view` GSettings schema (edited in
//! ragnar-settings). Read fresh on every menu so changes apply
//! immediately; defaults apply when the schema is not installed.

use gio::prelude::*;

const SCHEMA: &str = "org.lieraos.ragnar.view";

/// The view's settings, or None when the schema (with all keys this
/// version needs) is not installed — `Settings::new` would abort.
pub fn settings() -> Option<gio::Settings> {
    let schema = gio::SettingsSchemaSource::default()?.lookup(SCHEMA, true)?;
    let ok = ["delete-action", "context-menu-hidden", "arrange", "sort-by"]
        .iter()
        .all(|k| schema.has_key(k));
    ok.then(|| gio::Settings::new(SCHEMA))
}

/// What the delete entry does.
#[derive(Clone, Copy, PartialEq)]
pub enum DeleteAction {
    /// Remove permanently (after confirmation) — the default.
    Delete,
    /// Move to the trash.
    Trash,
}

/// Snapshot of the context-menu preferences.
pub struct Prefs {
    pub delete: DeleteAction,
    hidden: Vec<String>,
}

impl Prefs {
    /// Read the current preferences.
    pub fn load() -> Self {
        match settings() {
            Some(s) => Self {
                delete: if s.string("delete-action") == "trash" {
                    DeleteAction::Trash
                } else {
                    DeleteAction::Delete
                },
                hidden: s
                    .strv("context-menu-hidden")
                    .iter()
                    .map(|x| x.to_string())
                    .collect(),
            },
            None => Self {
                delete: DeleteAction::Delete,
                hidden: Vec::new(),
            },
        }
    }

    /// Whether the entry `id` should appear in the menu.
    pub fn shows(&self, id: &str) -> bool {
        !self.hidden.iter().any(|h| h == id)
    }
}
