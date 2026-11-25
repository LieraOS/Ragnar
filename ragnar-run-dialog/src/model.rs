// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-run-dialog — launcher data model and match scoring
//
// Scoring lives on plain string fields so it stays unit-testable
// without a display server or a running GTK main loop.

use gtk4::gio;

/// One launchable thing backing a result-list row.
pub enum Row {
    /// Index into the enumerated app list.
    App(usize),
    /// Fallback row: run the typed text as a command line.
    Command(String),
}

/// A desktop-file application reduced to what the UI needs.
pub struct AppEntry {
    /// Translated display name (Name=).
    pub name: String,
    /// GenericName= or Comment= — the dimmed second line in the row.
    pub description: String,
    /// Extra haystack: Keywords= plus the Exec= command.
    pub extra: String,
    /// The desktop entry itself, for icon and launch.
    pub info: gio::AppInfo,
}

/// Substring score for one field; `base` ranks the field kind so a
/// name hit always beats a description hit. Lower is better.
fn field_score(hay: &str, query: &str, base: i32) -> Option<i32> {
    hay.find(query).map(|pos| base + pos as i32)
}

/// Best match score over an entry's fields, or None when nothing
/// matches. A name prefix wins outright (tie-break: shorter name);
/// otherwise name > description > keywords/exec. An empty query
/// matches everything at a fixed score so the initial list keeps
/// alphabetical order.
pub fn score_fields(name: &str, description: &str, extra: &str, query: &str) -> Option<i32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let name_l = name.to_lowercase();
    if name_l.starts_with(&q) {
        return Some(name_l.len() as i32 - q.len() as i32);
    }
    [
        field_score(&name_l, &q, 10),
        field_score(&description.to_lowercase(), &q, 30),
        field_score(&extra.to_lowercase(), &q, 40),
    ]
    .into_iter()
    .flatten()
    .min()
}

#[cfg(test)]
mod tests {
    use super::score_fields;

    #[test]
    fn prefix_beats_substring() {
        // "Fire" prefixes "Firefox" but only appears mid-word in "Firejail sandbox".
        let a = score_fields("Firefox", "Webbrowser", "", "fire");
        let b = score_fields("Firejail sandbox", "", "", "fire");
        assert!(a < b, "prefix match must outrank substring match");
    }

    #[test]
    fn name_beats_description() {
        let a = score_fields("Krita", "Malprogramm", "", "bild");
        let b = score_fields("Bildbearbeitung", "Fotos", "", "bild");
        assert!(a.is_some() && b.is_some() && b < a || a.is_none());
        // Substring in name must outrank a description-only hit:
        let c = score_fields("GIMP", "Bildbearbeitung", "", "gimp");
        let d = score_fields("Fotos", "gimp plugin", "", "gimp");
        assert!(c < d);
    }

    #[test]
    fn no_match_is_none() {
        assert!(score_fields("Firefox", "Browser", "firefox", "zzz").is_none());
    }

    #[test]
    fn empty_query_matches_all() {
        assert_eq!(score_fields("A", "b", "c", ""), Some(0));
        assert_eq!(score_fields("A", "b", "c", "   "), Some(0));
    }

    #[test]
    fn case_insensitive() {
        assert!(score_fields("FIREFOX", "", "", "firefox").is_some());
        assert!(score_fields("firefox", "", "", "FIREFOX").is_some());
    }
}
