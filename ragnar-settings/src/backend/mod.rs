//! Backend helpers: availability checks against the host system.

pub mod availability;

pub use availability::{satisfied, Requirement};
