// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — appearance sub-page: fonts
// Copyright (C) 2026 LieraOS Project
//
// FontDialogButton (native font picker with preview) drives
// font-name / document-font-name / monospace-font-name. Scaling,
// antialiasing and hinting map to the matching interface keys.

use gtk4::pango;
use gtk4::prelude::*;

const AA_OPTIONS: &[(&str, &str)] = &[
    ("Standard (grayscale)", "grayscale"),
    ("Subpixel (LCD)", "rgba"),
    ("None", "none"),
];

const HINTING_OPTIONS: &[(&str, &str)] = &[
    ("Slight", "slight"),
    ("Medium", "medium"),
    ("Full", "full"),
    ("None", "none"),
];

/// Build the fonts page.
pub fn build() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    page.set_margin_top(18);

    let iface = gtk4::gio::Settings::new("org.gnome.desktop.interface");

    page.append(&font_row(&iface, "Interface font", "font-name"));
    page.append(&font_row(&iface, "Document font", "document-font-name"));
    page.append(&font_row(&iface, "Monospace font", "monospace-font-name"));

    // Text scaling factor (0.5–2.0).
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some("Text size factor")));
    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.5, 2.0, 0.05);
    scale.set_value(iface.double("text-scaling-factor"));
    scale.set_hexpand(true);
    scale.set_digits(2);
    scale.set_value_pos(gtk4::PositionType::Right);
    {
        let iface = iface.clone();
        scale.connect_value_changed(move |s| {
            let _ = iface.set_double("text-scaling-factor", s.value());
        });
    }
    row.append(&scale);
    page.append(&row);

    page.append(&enum_row(
        &iface,
        "Antialiasing",
        "font-antialiasing",
        AA_OPTIONS,
    ));
    page.append(&enum_row(
        &iface,
        "Hinting",
        "font-hinting",
        HINTING_OPTIONS,
    ));

    page
}

/// Row with label + FontDialogButton bound to a "Family Size" string key.
fn font_row(iface: &gtk4::gio::Settings, label: &str, key: &'static str) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));

    let button = gtk4::FontDialogButton::new(Some(gtk4::FontDialog::new()));
    button.set_hexpand(true);
    button.set_halign(gtk4::Align::End);

    // "Sans 10" -> FontDesc
    let desc = pango::FontDescription::from_string(&iface.string(key));
    button.set_font_desc(&desc);

    {
        let iface = iface.clone();
        button.connect_font_desc_notify(move |b| {
            // Store as "Family NN" string.
            let Some(desc) = b.font_desc() else { return };
            let family = desc.family().map(|f| f.to_string()).unwrap_or_default();
            let size = desc.size() / pango::SCALE;
            let style = match desc.style() {
                pango::Style::Italic => " Italic",
                pango::Style::Oblique => " Oblique",
                _ => "",
            };
            let weight = if desc.weight() == pango::Weight::Bold {
                " Bold"
            } else {
                ""
            };
            let _ = iface.set_string(key, &format!("{}{}{} {}", family, weight, style, size));
        });
    }

    row.append(&button);
    row
}

/// Row with label + dropdown for a string-enum key.
fn enum_row(
    iface: &gtk4::gio::Settings,
    label: &str,
    key: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));

    let labels: Vec<&str> = options.iter().map(|(l, _)| *l).collect();
    let dd = gtk4::DropDown::from_strings(&labels);
    dd.set_halign(gtk4::Align::End);
    let cur = iface.string(key).to_string();
    if let Some(i) = options.iter().position(|(_, v)| *v == cur) {
        dd.set_selected(i as u32);
    }
    let iface = iface.clone();
    dd.connect_selected_notify(move |d| {
        let (_, value) = options[d.selected() as usize];
        let _ = iface.set_string(key, value);
    });
    row.append(&dd);
    row
}
