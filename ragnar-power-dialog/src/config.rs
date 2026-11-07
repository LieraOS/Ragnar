// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — visibility configuration
//
// The org.lieraos.ragnar.session schema is shipped by this package
// and shared with Ragnar Settings, whose Power panel writes the
// show-* keys. Missing schema (old package state) means "show
// everything" — a safe default, never a crash.

use gtk4::gio;
use gtk4::prelude::*;

/// GSettings schema id owned by this package.
const SCHEMA: &str = "org.lieraos.ragnar.session";

/// Look up the schema; None when it is not installed on this system.
fn settings() -> Option<gio::Settings> {
    gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(SCHEMA, true))
        .map(|_| gio::Settings::new(SCHEMA))
}

/// Whether an action id ("lock", "logout", …) should be offered in
/// the dialog. Falls back to `true` when the schema or key is absent.
pub fn visible(action_id: &str) -> bool {
    let Some(s) = settings() else { return true };
    let key = format!("show-{action_id}");
    let has_key = s
        .settings_schema()
        .map(|schema| schema.has_key(&key))
        .unwrap_or(false);
    has_key && s.boolean(&key)
}
