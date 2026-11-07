//! "Cloud" panel — manage cloud accounts (Nextcloud/ownCloud/WebDAV).
//!
//! Accounts live in the system keyring via the shared `liera-secrets`
//! crate (freedesktop Secret Service). Other components — ragnar-raven
//! for CalDAV sync, Valkyrie for WebDAV mounts — read the same store,
//! so this panel is the single entry point for cloud credentials.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation};
use liera_secrets::CloudAccount;

/// Build the cloud panel widget.
pub fn build() -> gtk4::Widget {
    let container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(24)
        .margin_end(24)
        .build();

    let heading = Label::builder()
        .label(gettext("Cloud accounts"))
        .xalign(0.0)
        .build();
    heading.add_css_class("title-3");
    container.append(&heading);

    let info = Label::builder()
        .label(gettext(
            "Accounts stored here are used by Raven (calendar sync) \
             and other LieraOS applications (e.g. WebDAV mounts).",
        ))
        .xalign(0.0)
        .wrap(true)
        .build();
    info.add_css_class("dim-label");
    container.append(&info);

    let list = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    container.append(&list);

    let add = Button::builder()
        .label(gettext("Add account"))
        .halign(gtk4::Align::Start)
        .build();
    add.add_css_class("suggested-action");
    container.append(&add);

    refresh(&list);
    let l = list.clone();
    add.connect_clicked(move |b| open_editor(b, &l));

    container.upcast()
}

/// Re-render the account rows.
fn refresh(list: &Box) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    let names = liera_secrets::list().unwrap_or_default();
    if names.is_empty() {
        let none = Label::builder()
            .label(gettext("No cloud accounts configured"))
            .xalign(0.0)
            .build();
        none.add_css_class("dim-label");
        list.append(&none);
        return;
    }
    for name in names {
        let row = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .build();
        let detail = match liera_secrets::get(&name) {
            Ok(Some(a)) => format!("{}  —  {}  ({})", a.service, a.url, a.user),
            _ => name.clone(),
        };
        let l = Label::builder()
            .label(detail)
            .xalign(0.0)
            .hexpand(true)
            .build();
        row.append(&l);
        let del = Button::from_icon_name("user-trash-symbolic");
        del.set_has_frame(false);
        del.set_tooltip_text(Some(&gettext("Remove account")));
        let (svc, lst) = (name.clone(), list.clone());
        del.connect_clicked(move |_| {
            let _ = liera_secrets::delete(&svc);
            refresh(&lst);
        });
        row.append(&del);
        list.append(&row);
    }
}

/// Small editor dialog: service id, server URL, user, password.
fn open_editor(anchor: &Button, list: &Box) {
    let win = gtk4::Window::builder()
        .title(gettext("Add cloud account"))
        .modal(true)
        .default_width(380)
        .build();
    if let Some(toplevel) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&toplevel));
    }

    let body = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let service = gtk4::Entry::builder()
        .placeholder_text(gettext("Service name (e.g. nextcloud)"))
        .build();
    let url = gtk4::Entry::builder()
        .placeholder_text(gettext("Server URL (WebDAV/CalDAV)"))
        .build();
    let user = gtk4::Entry::builder()
        .placeholder_text(gettext("User name"))
        .build();
    let pass = gtk4::PasswordEntry::builder().show_peek_icon(true).build();
    pass.set_property("placeholder-text", gettext("Password"));
    for e in [&service, &url, &user] {
        body.append(e);
    }
    body.append(&pass);

    let buttons = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .halign(gtk4::Align::End)
        .build();
    let cancel = Button::with_label(&gettext("Cancel"));
    let save = Button::with_label(&gettext("Save"));
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);
    body.append(&buttons);

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    save.connect_clicked({
        let (w, lst) = (win.clone(), list.clone());
        move |_| {
            let svc = service.text().trim().to_string();
            if svc.is_empty() {
                return;
            }
            let acc = CloudAccount {
                service: svc,
                url: url.text().trim().to_string(),
                user: user.text().trim().to_string(),
                password: pass.text().to_string(),
            };
            let _ = liera_secrets::store(&acc);
            refresh(&lst);
            w.close();
        }
    });

    win.set_child(Some(&body));
    win.present();
}
