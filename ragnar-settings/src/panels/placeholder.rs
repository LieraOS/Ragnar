// Copyright (C) Jörg Schröder
//! Placeholder shown for panel ids that have no implementation yet.
//!
//! This is *not* a shipped feature — it exists so the hub can already
//! display the full catalog while panels are being built one by one.
//! Each entry lands here until its real panel in
//! [`crate::panels`] is implemented.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};

/// Build the placeholder widget for a not-yet-implemented panel id.
pub fn build(id: &str) -> Option<gtk4::Widget> {
    let container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();
    let msg = if id == "software" {
        gettextrs::gettext("Coming soon")
    } else {
        format!(
            "{}: {}",
            gettextrs::gettext("This settings area is not implemented yet"),
            id
        )
    };
    container.append(&Label::new(Some(&msg)));
    Some(container.upcast())
}
