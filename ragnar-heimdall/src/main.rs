// Copyright (C) Jörg Schröder
//! Entry point: logging, gettext, application bootstrap.

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = ragnar_heimdall::i18n::init() {
        tracing::warn!("gettext initialization failed: {err:#}");
    }

    let code = ragnar_heimdall::app::run();
    std::process::exit(code);
}
