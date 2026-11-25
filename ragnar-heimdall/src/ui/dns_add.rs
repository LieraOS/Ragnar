// Copyright (C) Jörg Schröder
//! The "set DNS servers" form dialog of the DNS page, split out to
//! keep `dns.rs` small. Offers well-known public resolvers as
//! presets plus a free-form field for custom entries, then hands
//! the chosen server list back via `submit`.

use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{AlertDialog, ComboRow, EntryRow};
use gtk4::{ListBox, StringList};

/// Public resolvers offered as presets: `(label, servers)`. These
/// are real anycast services — no placeholders. "Custom" leaves the
/// entry field for manual addresses.
const PRESETS: &[(&str, &str)] = &[
    ("Cloudflare (1.1.1.1)", "1.1.1.1 1.0.0.1"),
    ("Google (8.8.8.8)", "8.8.8.8 8.8.4.4"),
    ("Quad9 (9.9.9.9)", "9.9.9.9 149.112.112.112"),
    ("Custom", ""),
];

/// Present the dialog on `parent`. `submit` receives the list of
/// chosen DNS server addresses (empty only when the user picked a
/// preset-less custom input and left it blank — prevented by the
/// validation anyway).
pub fn present<F: Fn(Vec<String>) + 'static>(
    parent: &impl IsA<gtk4::Widget>,
    ifname: &str,
    submit: F,
) {
    let dialog = AlertDialog::builder()
        .heading(format!(
            "{} ({ifname})",
            gettextrs::gettext("Set DNS servers")
        ))
        .body(gettextrs::gettext(
            "Pick a public resolver or type addresses separated by spaces.",
        ))
        .build();

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);

    let labels: Vec<&str> = PRESETS.iter().map(|(l, _)| *l).collect();
    let model = StringList::new(&[]);
    for label in &labels {
        model.append(label);
    }
    let preset = ComboRow::builder()
        .title(gettextrs::gettext("Resolver"))
        .model(&model)
        .build();
    list.append(&preset);

    let custom = EntryRow::builder()
        .title(gettextrs::gettext("Custom servers"))
        .build();
    custom.set_visible(false);
    list.append(&custom);

    dialog.set_extra_child(Some(&list));
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("apply", &gettextrs::gettext("Apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("apply"));
    dialog.set_close_response("cancel");
    // A preset is selected by default — always a valid state.
    dialog.set_response_enabled("apply", true);

    // Custom input only appears (and is required) when the "Custom"
    // preset is chosen — presets carry their own addresses.
    {
        let dialog = dialog.clone();
        let custom = custom.clone();
        preset.connect_selected_notify(move |combo| {
            let is_custom = PRESETS[combo.selected() as usize].1.is_empty();
            custom.set_visible(is_custom);
            dialog.set_response_enabled("apply", !is_custom || !custom.text().trim().is_empty());
        });
    }
    {
        let dialog = dialog.clone();
        custom.connect_changed(move |e| {
            dialog.set_response_enabled("apply", !e.text().trim().is_empty());
        });
    }

    let submit = Rc::new(submit);
    dialog.connect_response(None, move |_, response| {
        if response != "apply" {
            return;
        }
        let servers: Vec<String> = if PRESETS[preset.selected() as usize].1.is_empty() {
            custom
                .text()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        } else {
            PRESETS[preset.selected() as usize]
                .1
                .split_whitespace()
                .map(str::to_string)
                .collect()
        };
        if !servers.is_empty() {
            submit(servers);
        }
    });
    dialog.present(Some(parent));
}
