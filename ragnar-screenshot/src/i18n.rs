// SPDX-License-Identifier: GPL-3.0-or-later
// gettext wrapper — same pattern as the other Ragnar Rust tools.
use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};

pub fn init() {
    let dir = std::env::var("LOCALEDIR").unwrap_or_else(|_| "/usr/share/locale".into());
    setlocale(LocaleCategory::LcAll, "");
    let _ = bindtextdomain("ragnar-screenshot", &dir);
    let _ = bind_textdomain_codeset("ragnar-screenshot", "UTF-8");
    let _ = textdomain("ragnar-screenshot");
}

/// Translate a message — kept as a fn so call sites stay short.
pub fn tr(msg: &str) -> String {
    gettextrs::gettext(msg)
}
