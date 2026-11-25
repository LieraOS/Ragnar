// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-power-dialog — gettext localization
//
// Same convention as ragnar-taskbar: the msgid IS the German source
// string (this UI is German-first), so de_DE needs no catalog;
// po/en.po carries msgid → msgstr for English locales.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// Gettext textdomain — equals the .mo filename minus extension.
pub const DOMAIN: &str = "ragnar-power-dialog";

/// Bind the domain to the system locale dir and force UTF-8.
/// LOCALEDIR overrides the path for development runs.
pub fn init() {
    let dir = std::env::var("LOCALEDIR").unwrap_or_else(|_| "/usr/share/locale".into());
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, &dir);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Translate one msgid — single call site so xgettext stays a dumb
/// extractor (`--keyword=tr`).
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}
