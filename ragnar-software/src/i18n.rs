// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Internationalisation — standard gettext. The msgid IS the English
//! source string; de.po carries the German translation. Strings that
//! travel through variables (arrays, consts) must additionally be
//! marked with [`msgid`] so xgettext still extracts them.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

const DOMAIN: &str = "ragnar-software";

/// Bind the text domain and codeset; called once at startup.
pub fn init() {
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, "/usr/share/locale");
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Identity marker: returns `s` unchanged but lets xgettext extract
/// the literal when the call site uses a variable instead of a
/// string literal — run extraction with `--keyword=msgid`.
pub const fn msgid(s: &'static str) -> &'static str {
    s
}
