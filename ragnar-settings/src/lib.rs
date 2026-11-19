//! Ragnar Settings — native settings hub for the Ragnar desktop.
//!
//! The application is a categorized overview of settings areas. Each
//! tile either opens an embedded native panel ([`panels`]) or launches
//! an existing LieraOS application ([`launch`]). Entries come from a
//! static [`registry`] catalog filtered through per-entry
//! [`backend::Requirement`] checks so the same binary runs correctly
//! inside a Ragnar session and on a development machine without it.

pub mod advanced;
pub mod backend;
pub mod backup;
pub mod i18n;
pub mod launch;
pub mod panels;
pub mod registry;
pub mod ui;
