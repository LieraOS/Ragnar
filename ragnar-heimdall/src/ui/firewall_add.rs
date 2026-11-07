//! The "open or close a port" form dialog of the firewall page,
//! split out to keep `firewall.rs` small. Collects action, port,
//! protocol and an optional source restriction, then hands the
//! mutation back to the page via the `submit` callback.

use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{AlertDialog, ComboRow, EntryRow};
use gtk4::{ListBox, StringList};

/// Present the form dialog on `parent`. `submit` receives
/// `(allow, port, proto, from)` only when the user confirms with a
/// non-empty port field.
pub fn present<F: Fn(bool, String, &'static str, String) + 'static>(
    parent: &impl IsA<gtk4::Widget>,
    submit: F,
) {
    let dialog = AlertDialog::builder()
        .heading(gettextrs::gettext("Open or close a port"))
        .build();

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);

    let action = ComboRow::builder()
        .title(gettextrs::gettext("Action"))
        .model(&StringList::new(&[
            &gettextrs::gettext("Open (allow connections)"),
            &gettextrs::gettext("Block (deny connections)"),
        ]))
        .build();
    list.append(&action);

    let port = EntryRow::builder()
        .title(gettextrs::gettext("Port"))
        .build();
    list.append(&port);

    let proto = ComboRow::builder()
        .title(gettextrs::gettext("Protocol"))
        .model(&StringList::new(&["TCP + UDP", "TCP", "UDP"]))
        .build();
    list.append(&proto);

    let from = EntryRow::builder()
        .title(gettextrs::gettext("From (optional)"))
        .build();
    list.append(&from);

    dialog.set_extra_child(Some(&list));
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("add", &gettextrs::gettext("Add rule"));
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("add"));
    dialog.set_close_response("cancel");
    dialog.set_response_enabled("add", false);

    {
        let dialog = dialog.clone();
        port.connect_changed(move |e| {
            dialog.set_response_enabled("add", !e.text().trim().is_empty());
        });
    }

    let submit = Rc::new(submit);
    dialog.connect_response(None, move |_, response| {
        if response != "add" {
            return;
        }
        let proto = match proto.selected() {
            1 => "tcp",
            2 => "udp",
            _ => "any",
        };
        submit(
            action.selected() == 0,
            port.text().to_string(),
            proto,
            from.text().to_string(),
        );
    });
    dialog.present(Some(parent));
}
