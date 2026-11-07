//! Internationalisation — standard gettext. The msgid IS the German
//! source string (this UI is German-first), so de_DE needs no
//! catalog; po/en.po carries msgid → msgstr for English locales.
//! Catalogs install to /usr/share/locale/<lang>/LC_MESSAGES/
//! ragnar-raven.mo via the .deb; LC_MESSAGES selects the language.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// Textdomain — equals the .mo filename minus extension.
pub const DOMAIN: &str = "ragnar-raven";

/// Bind the domain to the system locale dir and force UTF-8 —
/// GTK4 expects UTF-8 everywhere.
pub fn init() {
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, "/usr/share/locale");
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Translate one msgid — the single call site every UI string goes
/// through, so xgettext stays a dumb extractor (`--keyword=tr`).
/// Falls back to the German source when no catalog exists.
pub fn tr(msg: &str) -> String {
    gettextrs::dgettext(DOMAIN, msg)
}
