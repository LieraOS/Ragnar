// SPDX-License-Identifier: GPL-3.0-or-later
//! Internationalisation — standard gettext. The msgid IS the English
//! source string; de.po carries the German translation. Strings that
//! travel through variables must additionally be marked with [`msgid`]
//! so xgettext still extracts them (`--keyword=msgid`).

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

const DOMAIN: &str = "ragnar-taskmanager";

/// Bind the text domain and codeset; called once at startup.
pub fn init() {
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, "/usr/share/locale");
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Identity marker: returns `s` unchanged but lets xgettext extract
/// the literal when the call site uses a variable — extraction is run
/// with `--keyword=msgid`.
pub const fn msgid(s: &'static str) -> &'static str {
    s
}
