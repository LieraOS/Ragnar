//! Localization via gettext.
//!
//! The textdomain is `ragnar-settings`; compiled catalogs are searched
//! in the system locale dir and in `LOCALEDIR` (useful for development
//! runs without installation).

use anyhow::{Context, Result};

/// Gettext textdomain for this application.
pub const TEXTDOMAIN: &str = "ragnar-settings";

/// Default installation location of the compiled catalogs.
const SYSTEM_LOCALEDIR: &str = "/usr/share/locale";

/// Initialize gettext: bind the textdomain and set UTF-8 codeset.
///
/// `LOCALEDIR` overrides the search path so development builds can
/// load catalogs straight from the working tree.
pub fn init() -> Result<()> {
    let localedir = std::env::var("LOCALEDIR").unwrap_or_else(|_| SYSTEM_LOCALEDIR.into());
    gettextrs::bindtextdomain(TEXTDOMAIN, &localedir)
        .with_context(|| format!("cannot bind textdomain {TEXTDOMAIN} to {localedir}"))?;
    gettextrs::bind_textdomain_codeset(TEXTDOMAIN, "UTF-8").context("cannot set UTF-8 codeset")?;
    gettextrs::textdomain(TEXTDOMAIN).context("cannot set textdomain")?;
    Ok(())
}
