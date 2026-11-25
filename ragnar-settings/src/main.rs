// Copyright (C) Jörg Schröder
//! Ragnar Settings entry point.
//!
//! Initializes logging and localization, then hands control to the
//! GTK application in [`ragnar_settings::app`].

mod app;

use ragnar_settings::i18n;

fn main() -> gtk4::glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = i18n::init() {
        tracing::warn!("localization unavailable, falling back to English: {err}");
    }

    app::run()
}
