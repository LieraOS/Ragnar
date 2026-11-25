// Copyright (C) Jörg Schröder
//! DHCP/static IPv4 choice dialog for one interface. `static`
//! exposes address+gateway fields; everything validates client-side
//! before the pkexec round-trip. Link-level extras (DNS, MTU) apply
//! in both modes.

use libadwaita as adw;

use adw::prelude::*;
use adw::ActionRow;

use crate::backend;

/// Present the IPv4 configuration dialog for `iface`.
pub fn ipv4_dialog(source: &ActionRow, iface: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(format!(
            "{} ({iface})",
            gettextrs::gettext("IPv4 configuration")
        ))
        .build();
    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    let mode = adw::ComboRow::builder()
        .title(gettextrs::gettext("Method"))
        .model(&gtk4::StringList::new(&[
            &gettextrs::gettext("Automatic (DHCP)"),
            &gettextrs::gettext("Static"),
        ]))
        .build();
    list.append(&mode);
    let addr = adw::EntryRow::builder()
        .title(gettextrs::gettext("Address/mask (e.g. 192.168.1.10/24)"))
        .build();
    addr.set_visible(false);
    let gw = adw::EntryRow::builder()
        .title(gettextrs::gettext("Gateway"))
        .build();
    gw.set_visible(false);
    list.append(&addr);
    list.append(&gw);
    let dns = adw::EntryRow::builder()
        .title(gettextrs::gettext(
            "DNS servers (comma-separated, optional)",
        ))
        .build();
    list.append(&dns);
    let mtu = adw::EntryRow::builder()
        .title(gettextrs::gettext("MTU (optional)"))
        .build();
    list.append(&mtu);
    dialog.set_extra_child(Some(&list));
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("apply", &gettextrs::gettext("Apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_close_response("cancel");
    {
        let addr = addr.clone();
        let gw = gw.clone();
        mode.connect_selected_notify(move |m| {
            let statik = m.selected() == 1;
            addr.set_visible(statik);
            gw.set_visible(statik);
        });
    }
    let ifn = iface.to_string();
    dialog.connect_response(None, move |_, response| {
        if response != "apply" {
            return;
        }
        let ifn = ifn.clone();
        let dns_v = dns.text().trim().to_string();
        let mtu_v = mtu.text().trim().parse::<u32>().ok();
        // Link-level extras apply in both modes — DNS via resolvectl,
        // MTU via ip link, independent of the address method.
        if !dns_v.is_empty() || mtu_v.is_some() {
            let ifn2 = ifn.clone();
            std::thread::spawn(move || {
                if !dns_v.is_empty() {
                    if let Err(err) = backend::set_dns(&ifn2, &dns_v) {
                        tracing::warn!("set dns failed: {err:#}");
                    }
                }
                if let Some(m) = mtu_v {
                    if let Err(err) = backend::set_mtu(&ifn2, m) {
                        tracing::warn!("set mtu failed: {err:#}");
                    }
                }
            });
        }
        if mode.selected() == 0 {
            std::thread::spawn(move || {
                if let Err(err) = backend::set_dhcp(&ifn) {
                    tracing::warn!("set dhcp failed: {err:#}");
                }
            });
        } else {
            let a = addr.text().trim().to_string();
            let g = gw.text().trim().to_string();
            if a.is_empty() || g.is_empty() {
                return;
            }
            std::thread::spawn(move || {
                if let Err(err) = backend::set_static(&ifn, &a, &g) {
                    tracing::warn!("set static failed: {err:#}");
                }
            });
        }
    });
    dialog.present(Some(source));
}
