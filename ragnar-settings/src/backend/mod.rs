// Copyright (C) Jörg Schröder
//! Backend helpers: availability checks against the host system.

pub mod availability;
pub mod compositor;

pub use availability::{satisfied, Requirement};
