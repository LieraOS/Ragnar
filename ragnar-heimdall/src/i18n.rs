// Copyright (C) Jörg Schröder
//! Gettext setup. The text domain is `ragnar-heimdall`; translations live in
//! the system locale directory unless `LOCALEDIR` points elsewhere
//! (used for development runs).

use anyhow::{Context, Result};

/// Text domain shared with the `.mo` file names.
const DOMAIN: &str = "ragnar-heimdall";

/// System locale directory used when `LOCALEDIR` is not set.
const DEFAULT_LOCALEDIR: &str = "/usr/share/locale";

/// Bind the domain and codeset so `gettext` calls resolve UTF-8.
pub fn init() -> Result<()> {
    let localedir = std::env::var("LOCALEDIR").unwrap_or_else(|_| DEFAULT_LOCALEDIR.into());
    gettextrs::bindtextdomain(DOMAIN, &localedir)
        .with_context(|| format!("bindtextdomain {localedir}"))?;
    gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8").context("bind_textdomain_codeset")?;
    gettextrs::textdomain(DOMAIN).context("textdomain")?;
    Ok(())
}
