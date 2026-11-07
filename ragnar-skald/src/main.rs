//! Skald — main entry point.
//!
//! Thin binary on top of the `ragnar-skald` library crate: initialize
//! logging, then run the GTK application.

use tracing_subscriber::EnvFilter;

use ragnar_skald::gui::application::SkaldApplication;

/// Application ID used for GTK and D-Bus registration.
const APP_ID: &str = "com.liera.ragnar-skald";

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    SkaldApplication::new(APP_ID).run();
}
