// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — appearance module: overview + sub-navigation
// Copyright (C) 2026 LieraOS Project
//
// Overview page with one button per category. Clicking pushes the
// matching sub-page on an internal Stack; every sub-page offers a
// "Back" button to return to the overview.

use gtk4::prelude::*;

use super::{cursor, fonts, icons, style, themes, wallpaper};

/// (button label, stack page name, builder).
const SECTIONS: &[(&str, &str)] = &[
    ("Style & Colors", "style"),
    ("GTK Theme", "themes"),
    ("Icon Theme", "icons"),
    ("Cursor", "cursor"),
    ("Fonts", "fonts"),
    ("Wallpaper", "wallpaper"),
];

/// Create the Appearance page: overview + internal sub-page stack.
pub fn create_page() -> gtk4::Widget {
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);

    // -- Overview ------------------------------------------------------
    let overview = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    overview.set_margin_start(24);
    overview.set_margin_end(24);
    overview.set_margin_top(24);

    let title = gtk4::Label::new(Some("Appearance"));
    title.add_css_class("title-1");
    title.set_halign(gtk4::Align::Start);
    overview.append(&title);

    for (label, page_name) in SECTIONS {
        let btn = gtk4::Button::new();
        let inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        inner.append(&gtk4::Label::new(Some(label)));
        let arrow = gtk4::Label::new(Some("›"));
        arrow.set_halign(gtk4::Align::End);
        arrow.set_hexpand(true);
        inner.append(&arrow);
        btn.set_child(Some(&inner));

        // Build the sub-page lazily on first navigation — eager
        // construction would scan theme dirs and decode wallpaper
        // images every time the panel opens.
        let stack = stack.clone();
        let page = page_name.to_string();
        let title = label.to_string();
        btn.connect_clicked(move |_| {
            if stack.child_by_name(&page).is_none() {
                stack.add_named(&section_page(&title, &page, &stack), Some(&page));
            }
            stack.set_visible_child_name(&page);
        });
        overview.append(&btn);
    }
    stack.add_named(&overview, Some("overview"));

    stack.set_visible_child_name("overview");
    stack.upcast()
}

/// Build one sub-page (back button, title, content) on demand.
fn section_page(label: &str, page_name: &str, stack: &gtk4::Stack) -> gtk4::Widget {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    page.set_margin_start(24);
    page.set_margin_end(24);
    page.set_margin_top(12);

    // Header: back button + page title.
    let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let back = gtk4::Button::with_label("‹ Back");
    {
        let stack = stack.clone();
        back.connect_clicked(move |_| stack.set_visible_child_name("overview"));
    }
    back.set_halign(gtk4::Align::Start);
    header.append(&back);
    let title = gtk4::Label::new(Some(label));
    title.add_css_class("heading");
    header.append(&title);
    page.append(&header);

    let body: gtk4::Widget = match page_name {
        "style" => style::build().upcast(),
        "themes" => themes::build().upcast(),
        "icons" => icons::build().upcast(),
        "cursor" => cursor::build().upcast(),
        "fonts" => fonts::build().upcast(),
        "wallpaper" => wallpaper::build().upcast(),
        _ => gtk4::Label::new(None).upcast(),
    };
    page.append(&body);
    page.upcast()
}
