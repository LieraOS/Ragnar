//! The "add a route" form dialog of the routing page, split out to
//! keep `routing.rs` small. Collects destination, next-hop gateway,
//! outgoing interface and an optional metric, then hands the values
//! back to the page via the `submit` callback.

use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{AlertDialog, ComboRow, EntryRow};
use gtk4::{ListBox, StringList};

use crate::backend;

/// Present the form dialog on `parent`. `submit` receives
/// `(destination, gateway, dev, metric)` only when the user confirms
/// with a non-empty destination and at least a gateway or interface.
pub fn present<F: Fn(String, String, String, String) + 'static>(
    parent: &impl IsA<gtk4::Widget>,
    submit: F,
) {
    let dialog = AlertDialog::builder()
        .heading(gettextrs::gettext("Add route"))
        .body(gettextrs::gettext(
            "Destination as IP or CIDR (e.g. 10.0.0.0/8 or default).",
        ))
        .build();

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);

    let dst = EntryRow::builder()
        .title(gettextrs::gettext("Destination"))
        .build();
    list.append(&dst);

    let gw = EntryRow::builder()
        .title(gettextrs::gettext("Gateway (optional)"))
        .build();
    list.append(&gw);

    // Interface choices come from the live snapshot — offering only
    // real names prevents typos that `ip` would reject anyway.
    let dev_model = StringList::new(&[]);
    dev_model.append(&gettextrs::gettext("(any)"));
    if let Ok(ifaces) = backend::collect_interfaces() {
        for i in &ifaces {
            dev_model.append(&i.name);
        }
    }
    let dev = ComboRow::builder()
        .title(gettextrs::gettext("Interface (optional)"))
        .model(&dev_model)
        .build();
    list.append(&dev);

    let metric = EntryRow::builder()
        .title(gettextrs::gettext("Metric (optional)"))
        .build();
    list.append(&metric);

    dialog.set_extra_child(Some(&list));
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("add", &gettextrs::gettext("Add route"));
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("add"));
    dialog.set_close_response("cancel");
    dialog.set_response_enabled("add", false);

    // Enable Add only when the minimum is met: destination plus a
    // gateway or a chosen interface — an anchorless route is always
    // rejected by the kernel, so the dialog fails fast instead.
    let validate = Rc::new({
        let dialog = dialog.clone();
        let dst = dst.clone();
        let gw = gw.clone();
        let dev = dev.clone();
        move || {
            let ok = !dst.text().trim().is_empty()
                && (!gw.text().trim().is_empty() || dev.selected() > 0);
            dialog.set_response_enabled("add", ok);
        }
    });
    {
        let v1 = Rc::clone(&validate);
        dst.connect_changed(move |_| v1());
        let v2 = Rc::clone(&validate);
        gw.connect_changed(move |_| v2());
        dev.connect_selected_notify(move |_| validate());
    }

    let submit = Rc::new(submit);
    dialog.connect_response(None, move |_, response| {
        if response != "add" {
            return;
        }
        let dev_name = if dev.selected() == 0 {
            String::new()
        } else {
            dev.selected_item()
                .and_then(|o| o.downcast::<gtk4::StringObject>().ok())
                .map(|s| s.string().to_string())
                .unwrap_or_default()
        };
        submit(
            dst.text().trim().to_string(),
            gw.text().trim().to_string(),
            dev_name,
            metric.text().trim().to_string(),
        );
    });
    dialog.present(Some(parent));
}
