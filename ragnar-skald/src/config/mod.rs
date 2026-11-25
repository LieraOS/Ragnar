// Copyright (C) Jörg Schröder
//! Config Module
//!
//! Application configuration and persistence.

pub mod persistence;
pub mod settings;

pub use persistence::ConfigPersistence;
pub use settings::AppSettings;
