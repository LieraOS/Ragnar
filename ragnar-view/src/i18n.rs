// Copyright (C) Jörg Schröder
//! Gettext-backed translations, German catalog as the reference.

/// Translate a message id via the app's text domain.
pub fn tr(msg: &str) -> String {
    gettextrs::gettext(msg)
}
