// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — gettext localization
//
// Same convention as the other Ragnar tools: the msgid IS the German
// source string (this UI is German-first), so de_DE needs no
// catalog; po/en.po carries msgid → msgstr for English locales.
//
// The saved language preference overrides the system locale: it is
// applied through the LANGUAGE env var BEFORE any gettext call, so
// a restart switches the whole UI — same model the design used
// (its translator reload also required a restart).

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};

/// Gettext textdomain — equals the.mo filename minus extension.
pub const DOMAIN: &str = "ragnar-sap";

/// Apply the stored language preference, then bind the domain.
/// MUST run before the first tr call.
pub fn init() {
 // Saved preference wins over the system locale. "system" (the
 // default) leaves everything untouched.
 let lang = crate::core::preferences::Preferences::load().get().language;
 if lang != "system" {
 unsafe { std::env::set_var("LANGUAGE", &lang) }
 }
 let dir = std::env::var("LOCALEDIR").unwrap_or_else(|_| "/usr/share/locale".into());
 let _ = setlocale(LocaleCategory::LcAll, "");
 let _ = textdomain(DOMAIN);
 let _ = bindtextdomain(DOMAIN, &dir);
 let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Translate one msgid — single call site so xgettext stays a dumb
/// extractor (`--keyword=tr`).
pub fn tr(msgid: &str) -> String {
 gettextrs::gettext(msgid)
}

/// Switch the UI language at runtime: updates the LANGUAGE override
/// and forces a locale refresh so every subsequent tr call returns
/// the new language immediately (glibc gettext re-reads LANGUAGE on
/// each lookup — verified). Callers must rebuild visible widgets.
pub fn set_language(lang: &str) {
 unsafe {
 match lang {
 "en" | "de" => std::env::set_var("LANGUAGE", lang),
 _ => std::env::remove_var("LANGUAGE"), // system → locale
 }
 }
 let _ = setlocale(LocaleCategory::LcAll, "");
}
