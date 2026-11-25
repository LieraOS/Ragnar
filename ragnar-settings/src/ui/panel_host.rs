// Copyright (C) Jörg Schröder
//! Hosts an embedded panel widget inside a stack page.

use gtk4::prelude::*;
use gtk4::ScrolledWindow;

/// Wrap `content` in a scrolled container with standard page margins.
/// Title and back button live in the persistent window header, so no
/// header is rendered here.
pub fn wrap(content: gtk4::Widget) -> ScrolledWindow {
    let page = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(24)
        .margin_end(24)
        .build();
    page.append(&content);

    ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&page)
        .vexpand(true)
        .build()
}
