// Internationalisation — standard gettext. The msgid IS the German
// source string (this UI was German-first), so de_DE needs no
// catalog; po/en.po carries msgid → msgstr for English locales.
// Catalogs install to /usr/share/locale/<lang>/LC_MESSAGES/
// ragnar-taskbar.mo via the .deb; LC_MESSAGES selects the language.
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// Textdomain — equals the .mo filename minus extension.
pub const DOMAIN: &str = "ragnar-taskbar";

/// Bind the domain to the system locale dir and force UTF-8 —
/// GTK4 expects UTF-8 everywhere.
pub fn init() {
    let _ = textdomain(DOMAIN);
    let _ = bindtextdomain(DOMAIN, "/usr/share/locale");
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Translate one msgid — the single call site every UI string goes
/// through, so xgettext stays a dumb extractor (`--keyword=tr`).
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}

/// Translated format: the msgid carries `{}` placeholders filled
/// left-to-right — translators keep the same count and order.
pub fn trf(msgid: &str, args: &[&str]) -> String {
    let mut out = tr(msgid);
    for a in args {
        out = out.replacen("{}", a, 1);
    }
    out
}
