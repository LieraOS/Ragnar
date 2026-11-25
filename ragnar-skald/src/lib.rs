// Copyright (C) Jörg Schröder
//! Skald - Audio Control Application
//!
//! Native PipeWire audio control with a GTK4 frontend, designed for
//! LieraOS / Wayland. The `pipewire` module runs the real backend on a
//! dedicated worker thread; `gui` is the frontend; `audio` and `config`
//! hold settings models and persistence.

pub mod audio;
pub mod config;
pub mod fxdaemon;
pub mod gui;
pub mod pipewire;
