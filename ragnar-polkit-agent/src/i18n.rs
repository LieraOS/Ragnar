// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-polkit-agent — gettext localization
//
// Same convention as the other Ragnar crates: the msgid IS the
// German source string (German-first UI), po/en.po carries the
// English translations.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// Gettext textdomain — equals the .mo filename minus extension.
pub const DOMAIN: &str = "ragnar-polkit-agent";

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
