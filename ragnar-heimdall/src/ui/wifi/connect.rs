// Copyright (C) Jörg Schröder
//! Connect and disconnect flows for the Wi-Fi section: password
//! dialog, worker-thread `Network.Connect`, and the explicit
//! user-triggered `Station.Disconnect`.

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{AlertDialog, PasswordEntryRow, ResponseAppearance};

use crate::backend::{self, WifiNetwork};

use super::{render, Section, POLL_MS};

/// Entry point for a click on a network row: secured networks open a
/// password dialog, open networks connect right away.
pub(super) fn begin_connect(ui: Rc<Section>, net: WifiNetwork) {
    if ui.connecting.borrow().is_some() {
        return;
    }
    if net.secured {
        password_dialog(ui, net);
    } else {
        start_connect(ui, net, None);
    }
}

/// Password prompt for a secured network. An empty password is
/// allowed: the backend then tries previously saved credentials.
fn password_dialog(ui: Rc<Section>, net: WifiNetwork) {
    let dialog = AlertDialog::builder()
        .heading(format!(
            "{} “{}”",
            gettextrs::gettext("Connect to"),
            net.ssid
        ))
        .body(gettextrs::gettext(
            "Enter the Wi-Fi password. Leave it empty to try saved credentials.",
        ))
        .build();

    // PasswordEntryRow is a listbox row — it must sit inside a
    // GtkListBox or GTK warns and the field may not behave correctly.
    let entry = PasswordEntryRow::builder()
        .title(gettextrs::gettext("Password"))
        .build();
    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.append(&entry);
    dialog.set_extra_child(Some(&list));

    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("connect", &gettextrs::gettext("Connect"));
    dialog.set_response_appearance("connect", ResponseAppearance::Suggested);
    dialog.set_default_response(Some("connect"));
    dialog.set_close_response("cancel");

    let ui2 = ui.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "connect" {
            start_connect(ui2.clone(), net.clone(), Some(entry.text().to_string()));
        }
    });
    dialog.present(Some(&ui.group));
}

/// Run the connect in a worker thread and update the section when
/// the result arrives.
fn start_connect(ui: Rc<Section>, net: WifiNetwork, password: Option<String>) {
    *ui.connecting.borrow_mut() = Some(net.ssid.clone());
    *ui.notice.borrow_mut() = None;
    render(&ui);

    let (tx, rx) = mpsc::channel();
    let iface = ui.iface.clone();
    std::thread::spawn(move || {
        let result = backend::wifi_connect(&iface, &net, password.as_deref());
        if result.is_ok() {
            backend::acquire_dhcp(&iface);
        }
        let _ = tx.send((net, result));
    });

    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok((net, result)) => {
                *ui.connecting.borrow_mut() = None;
                match result {
                    Ok(outcome) => {
                        *ui.connected.borrow_mut() = Some(net.ssid.clone());
                        let mut notes = Vec::new();
                        if !outcome.persisted {
                            notes.push(gettextrs::gettext(
                                "the network could not be saved permanently",
                            ));
                        }
                        if !outcome.ip_acquired {
                            notes.push(gettextrs::gettext(
                                "no IP address received yet — DHCP may still be running",
                            ));
                        }
                        if !notes.is_empty() {
                            *ui.notice.borrow_mut() = Some(format!(
                                "{}: {}",
                                gettextrs::gettext("Connected"),
                                notes.join("; ")
                            ));
                        }
                    }
                    Err(err) => {
                        tracing::warn!("wifi connect failed: {err:#}");
                        *ui.notice.borrow_mut() = Some(format!("{err:#}"));
                    }
                }
                render(&ui);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Drop the current association on the user's explicit request.
/// iwd keeps the stored profile, so reconnecting reuses it.
pub(super) fn start_disconnect(ui: Rc<Section>) {
    let (tx, rx) = mpsc::channel();
    let iface = ui.iface.clone();
    std::thread::spawn(move || {
        let _ = tx.send(backend::wifi_disconnect(&iface));
    });

    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(Ok(())) => {
                *ui.connected.borrow_mut() = None;
                render(&ui);
                gtk4::glib::ControlFlow::Break
            }
            Ok(Err(err)) => {
                tracing::warn!("wifi disconnect failed: {err:#}");
                *ui.notice.borrow_mut() = Some(format!("{err:#}"));
                render(&ui);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}
