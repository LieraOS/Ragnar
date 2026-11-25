// Copyright (C) Jörg Schröder
//! A single hub tile: icon, title and subtitle inside a button.

use gtk4::prelude::*;
use gtk4::{Box, Button, Image, Label, Orientation};

use crate::registry::Entry;

/// Build a tile button for `entry` and invoke `on_click` on activation.
///
/// Titles and subtitles are translated here — the registry stores
/// English source strings only.
pub fn build<F>(entry: &Entry, on_click: F) -> Button
where
    F: Fn() + 'static,
{
    let icon = Image::builder()
        .icon_name(entry.icon)
        .pixel_size(32)
        .build();
    let title = Label::builder()
        .label(gettextrs::gettext(entry.title))
        .wrap(false)
        .xalign(0.0)
        .build();
    title.add_css_class("heading");
    let subtitle = Label::builder()
        .label(gettextrs::gettext(entry.subtitle))
        .wrap(true)
        .xalign(0.0)
        .build();
    subtitle.add_css_class("dim-label");

    let text = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .valign(gtk4::Align::Center)
        .build();
    text.append(&title);
    text.append(&subtitle);

    let content = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    content.append(&icon);
    content.append(&text);

    let button = Button::builder().child(&content).build();
    button.add_css_class("flat");
    button.connect_clicked(move |_| on_click());
    button
}
