// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — workflow layer: sessions, scan/print/scan-to-print
// state machines, event bus.

// The whole layer is API surface for the GTK UI phases — nothing
// in main.rs consumes it yet, so allow dead code module-wide
// instead of annotating every item.
#[allow(dead_code)]
pub mod event_bus;
#[allow(dead_code)]
pub mod print;
#[allow(dead_code)]
pub mod scan;
#[allow(dead_code)]
pub mod session;
