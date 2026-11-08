// SPDX-License-Identifier: GPL-3.0-or-later
//! Internationalisation — standard gettext. The msgid IS the German
//! source string; en.po carries the English translation.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

const DOMAIN: &str = "ragnar-software";

/// Bind the text domain and codeset; called once at startup.
pub fn init() {
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, "/usr/share/locale");
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}
