// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — appearance sub-page: style + accent colors
// Copyright (C) 2026 LieraOS Project
//
// Dark/light mode (color-scheme + ragnar panel dark-theme) and the
// accent color. GNOME's accent-color enum gives the fixed palette;
// "Custom" writes an override into ~/.config/gtk-4.0/gtk.css.

use gtk4::prelude::*;
use std::fs;

/// (label, gsettings enum value, css color for the button swatch).
const ACCENTS: &[(&str, &str, &str)] = &[
    ("Blue", "blue", "#3584e4"),
    ("Teal", "teal", "#2190a4"),
    ("Green", "green", "#3a944a"),
    ("Yellow", "yellow", "#c88800"),
    ("Orange", "orange", "#ed5b00"),
    ("Red", "red", "#e62d42"),
    ("Pink", "pink", "#d56199"),
    ("Purple", "purple", "#9141ac"),
    ("Slate", "slate", "#6f8396"),
];

const STYLE_LABELS: &[&str] = &["System", "Dark", "Light"];
const STYLE_VALUES: &[&str] = &["default", "prefer-dark", "prefer-light"];

/// Build the style page (dark mode, accent palette, custom color).
pub fn build() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 18);
    page.set_margin_top(18);

    let iface = gtk4::gio::Settings::new("org.gnome.desktop.interface");

    // -- Dark / light mode ----------------------------------------------
    page.append(&section("Mode"));
    let dd = gtk4::DropDown::from_strings(STYLE_LABELS);
    let cur = iface.string("color-scheme").to_string();
    let idx = STYLE_VALUES.iter().position(|v| *v == cur).unwrap_or(0);
    dd.set_selected(idx as u32);
    let iface_mode = iface.clone();
    dd.connect_selected_notify(move |d| {
        let value = STYLE_VALUES[d.selected() as usize];
        let _ = iface_mode.set_string("color-scheme", value);
        // Keep the ragnar panel theme in sync.
        let panel = gtk4::gio::Settings::new("org.lieraos.ragnar.panel");
        let _ = panel.set_boolean("dark-theme", value != "prefer-light");
    });
    page.append(&dd);

    // -- Accent color ----------------------------------------------------
    page.append(&section("Accent color"));
    let flow = gtk4::FlowBox::new();
    flow.set_max_children_per_line(9);
    flow.set_selection_mode(gtk4::SelectionMode::None);

    for (label, value, color) in ACCENTS {
        let btn = gtk4::Button::new();
        btn.set_tooltip_text(Some(label));
        btn.set_size_request(36, 36);
        // Colored swatch via per-widget CSS provider. style_context is
        // deprecated since GTK 4.10 but remains the only API for
        // widget-scoped styling.
        let css = gtk4::CssProvider::new();
        css.load_from_data(&format!(
            "button {{ background: {}; border-radius: 50%; min-width: 0; min-height: 0; }}",
            color
        ));
        #[allow(deprecated)]
        btn.style_context()
            .add_provider(&css, gtk4::STYLE_PROVIDER_PRIORITY_USER);

        let iface = iface.clone();
        let value = value.to_string();
        btn.connect_clicked(move |_| {
            let _ = iface.set_string("accent-color", &value);
        });
        flow.append(&btn);
    }
    page.append(&flow);

    // Custom color → hex written to ~/.config/gtk-4.0/gtk.css.
    let custom_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    custom_row.append(&gtk4::Label::new(Some("Custom accent")));
    let custom = gtk4::ColorDialogButton::new(None);
    custom.set_halign(gtk4::Align::End);
    custom.set_hexpand(true);
    custom.connect_rgba_notify(move |b| {
        let rgba = b.rgba();
        let hex = format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.red() * 255.0) as u8,
            (rgba.green() * 255.0) as u8,
            (rgba.blue() * 255.0) as u8
        );
        write_custom_accent(&hex);
    });
    custom_row.append(&custom);
    page.append(&custom_row);

    let hint = gtk4::Label::new(Some(
        "Custom accents apply to GTK4 apps; the palette presets also style libadwaita apps.",
    ));
    hint.add_css_class("dim-label");
    hint.set_wrap(true);
    page.append(&hint);

    page
}

/// Write a user-level GTK4 accent override to ~/.config/gtk-4.0/gtk.css.
fn write_custom_accent(hex: &str) {
    let home = match std::env::var_os("HOME") {
        Some(h) => std::path::PathBuf::from(h),
        None => return,
    };
    let dir = home.join(".config/gtk-4.0");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("gtk.css");

    // Replace an existing accent line, append otherwise.
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut out = String::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line.contains("@accent_color") {
            out.push_str(&format!("@define-color accent_color {};\n", hex));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("@define-color accent_color {};\n", hex));
    }
    let _ = fs::write(&path, out);
}

/// Bold left-aligned section title.
fn section(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.add_css_class("heading");
    l.set_halign(gtk4::Align::Start);
    l
}
