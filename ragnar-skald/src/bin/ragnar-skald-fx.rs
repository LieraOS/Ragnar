// Copyright (C) Jörg Schröder
//! `ragnar-skald-fx` — headless effect-chain daemon binary.
//!
//! Thin entry point on top of `ragnar_skald::fxdaemon::run()`; the daemon
//! owns the PipeWire filter-chain modules so configured effects stay
//! active system-wide while the GUI window is closed.

fn main() {
    std::process::exit(ragnar_skald::fxdaemon::run());
}
