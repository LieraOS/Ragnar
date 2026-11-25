// Copyright (C) Jörg Schröder
//! Per-network properties dialog — the full iwd profile surface for
//! one SSID: AutoConnect and Hidden from [Settings], the WPA
//! passphrase from [Security], and an optional static IPv4 block
//! ([IPv4] Address/Netmask/Gateway/DNS) — per-network static config
//! iwd applies on connect without touching the global setup.

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use adw::prelude::*;
use adw::{ActionRow, ComboRow, EntryRow, PreferencesGroup, SwitchRow};
use libadwaita as adw;

use super::{render, Section};
use crate::backend::wifi::{profile_load, profile_save, Profile};

/// Open the properties window for `ssid`.
pub fn present(ui: &Rc<Section>, ssid: String) {
    let win = adw::Window::builder()
        .title(format!(
            "{} — {ssid}",
            gettextrs::gettext("Network properties")
        ))
        .default_width(560)
        .default_height(620)
        .modal(true)
        .build();
    if let Some(top) = ui
        .group
        .root()
        .and_then(|r| r.downcast::<gtk4::Window>().ok())
    {
        win.set_transient_for(Some(&top));
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    win.set_content(Some(&view));

    let page = adw::PreferencesPage::new();
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&page)
        .vexpand(true)
        .build();
    let spinner_row = ActionRow::builder()
        .title(gettextrs::gettext("Loading profile…"))
        .build();
    spinner_row.add_suffix(&adw::Spinner::new());
    let load_group = PreferencesGroup::new();
    load_group.add(&spinner_row);
    page.add(&load_group);
    view.set_content(Some(&scroll));
    win.present();

    // Profile load needs the pkexec helper — off the main thread.
    let (tx, rx) = mpsc::channel();
    let ssid2 = ssid.clone();
    std::thread::spawn(move || {
        let _ = tx.send(profile_load(&ssid2));
    });
    let w = win.clone();
    let u = ui.clone();
    gtk4::glib::timeout_add_local(Duration::from_millis(150), move || match rx.try_recv() {
        Ok(Ok(profile)) => {
            page.remove(&load_group);
            build_form(&w, &u, &ssid, profile, &page);
            gtk4::glib::ControlFlow::Break
        }
        Ok(Err(e)) => {
            load_group.set_description(Some(&format!("{e:#}")));
            gtk4::glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
    });
}

/// Render the editable form once the profile arrives.
fn build_form(
    win: &adw::Window,
    ui: &Rc<Section>,
    ssid: &str,
    p: Profile,
    page: &adw::PreferencesPage,
) {
    // --- General ---
    let gen = PreferencesGroup::builder()
        .title(gettextrs::gettext("General"))
        .build();
    let autoconn = SwitchRow::builder()
        .title(gettextrs::gettext("Connect automatically"))
        .subtitle(gettextrs::gettext(
            "Join this network whenever it is in range.",
        ))
        .active(p.get("Settings", "AutoConnect") != Some("false"))
        .build();
    gen.add(&autoconn);
    let hidden = SwitchRow::builder()
        .title(gettextrs::gettext("Hidden network"))
        .subtitle(gettextrs::gettext(
            "The SSID is not broadcast — connect by name.",
        ))
        .active(p.get("Settings", "Hidden") == Some("true"))
        .build();
    gen.add(&hidden);
    let askpw = SwitchRow::builder()
        .title(gettextrs::gettext("Always ask for passphrase"))
        .subtitle(gettextrs::gettext(
            "The key stays stored but must be confirmed on every connect.",
        ))
        .active(p.get("Settings", "AlwaysAskForPassphrase") == Some("true"))
        .build();
    gen.add(&askpw);
    let ipv6 = ComboRow::builder()
        .title(gettextrs::gettext("IPv6"))
        .model(&gtk4::StringList::new(&[
            &gettextrs::gettext("Automatic"),
            &gettextrs::gettext("Enabled"),
            &gettextrs::gettext("Disabled"),
        ]))
        .selected(match p.get("IPv6", "Enabled") {
            Some("true") => 1,
            Some("false") => 2,
            _ => 0,
        })
        .build();
    gen.add(&ipv6);
    page.add(&gen);

    // --- Security (psk / enterprise 802.1x) ---
    let suffix = p.file.as_deref().unwrap_or(".psk").to_string();
    let is_psk = suffix.ends_with(".psk");
    let is_8021x = suffix.ends_with(".8021x");
    let pass = EntryRow::builder()
        .title(gettextrs::gettext("Passphrase"))
        .text(p.get("Security", "Passphrase").unwrap_or_default())
        .build();
    // 802.1X EAP fields — PEAP and TTLS cover the common enterprise
    // deployments; phase-2 method/identity/password plus outer
    // identity and optional CA cert path.
    let eap_method = ComboRow::builder()
        .title(gettextrs::gettext("EAP method"))
        .model(&gtk4::StringList::new(&["PEAP", "TTLS"]))
        .selected(u32::from(p.get("Security", "EAP-Method") == Some("TTLS")))
        .build();
    let eap_id = EntryRow::builder()
        .title(gettextrs::gettext("Identity (outer)"))
        .text(p.get("Security", "EAP-Identity").unwrap_or_default())
        .build();
    let eap_p2m = ComboRow::builder()
        .title(gettextrs::gettext("Phase-2 method"))
        .model(&gtk4::StringList::new(&["MSCHAPV2", "PAP", "CHAP"]))
        .selected(match p.get("Security", "EAP-Phase2-Method") {
            Some("PAP") => 1,
            Some("CHAP") => 2,
            _ => 0,
        })
        .build();
    let eap_p2id = EntryRow::builder()
        .title(gettextrs::gettext("Username (inner)"))
        .text(p.get("Security", "EAP-Phase2-Identity").unwrap_or_default())
        .build();
    let eap_p2pw = adw::PasswordEntryRow::builder()
        .title(gettextrs::gettext("Password (inner)"))
        .build();
    eap_p2pw.set_text(p.get("Security", "EAP-Phase2-Password").unwrap_or_default());
    let eap_ca = EntryRow::builder()
        .title(gettextrs::gettext("CA certificate (optional path)"))
        .text(p.get("Security", "EAP-CACert").unwrap_or_default())
        .build();
    if is_psk || is_8021x {
        let sec = PreferencesGroup::builder()
            .title(gettextrs::gettext("Security"))
            .build();
        if is_psk {
            sec.add(&pass);
        } else {
            sec.add(&eap_method);
            sec.add(&eap_id);
            sec.add(&eap_p2m);
            sec.add(&eap_p2id);
            sec.add(&eap_p2pw);
            sec.add(&eap_ca);
        }
        page.add(&sec);
    }

    // --- IPv4 per network ---
    let ipg = PreferencesGroup::builder()
        .title(gettextrs::gettext("IPv4 (this network)"))
        .description(gettextrs::gettext(
            "Static address used only on this network — everything else stays DHCP.",
        ))
        .build();
    let method = ComboRow::builder()
        .title(gettextrs::gettext("Method"))
        .model(&gtk4::StringList::new(&[
            "DHCP",
            &gettextrs::gettext("Static"),
        ]))
        .selected(u32::from(p.get("IPv4", "Address").is_some()))
        .build();
    ipg.add(&method);
    let addr = EntryRow::builder()
        .title(gettextrs::gettext("Address"))
        .text(p.get("IPv4", "Address").unwrap_or_default())
        .build();
    let mask = EntryRow::builder()
        .title(gettextrs::gettext("Netmask"))
        .text(p.get("IPv4", "Netmask").unwrap_or_default())
        .build();
    let gw = EntryRow::builder()
        .title(gettextrs::gettext("Gateway"))
        .text(p.get("IPv4", "Gateway").unwrap_or_default())
        .build();
    let dns = EntryRow::builder()
        .title(gettextrs::gettext("DNS (comma-separated)"))
        .text(p.get("IPv4", "DNS").unwrap_or_default())
        .build();
    for e in [&addr, &mask, &gw, &dns] {
        e.set_sensitive(method.selected() == 1);
        ipg.add(e);
    }
    {
        let (a, m, g, d) = (addr.clone(), mask.clone(), gw.clone(), dns.clone());
        method.connect_selected_notify(move |c| {
            for e in [&a, &m, &g, &d] {
                e.set_sensitive(c.selected() == 1);
            }
        });
    }
    page.add(&ipg);

    // --- Save / Cancel ---
    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    btns.set_margin_top(8);
    btns.set_margin_bottom(12);
    btns.set_margin_end(12);
    let cancel = gtk4::Button::with_label(&gettextrs::gettext("Cancel"));
    let save = gtk4::Button::with_label(&gettextrs::gettext("Save"));
    save.add_css_class("suggested-action");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let (w, u) = (win.clone(), ui.clone());
        let ssid = ssid.to_string();
        save.connect_clicked(move |_| {
            // MERGE, not overwrite: clone the loaded profile so keys
            // the form does not cover (hand-edited iwd options,
            // EAP extras, comments-as-values) survive a save.
            let mut p2 = p.clone();
            p2.set(
                "Settings",
                "AutoConnect",
                if autoconn.is_active() {
                    "true"
                } else {
                    "false"
                },
            );
            for (on, key) in [
                (hidden.is_active(), "Hidden"),
                (askpw.is_active(), "AlwaysAskForPassphrase"),
            ] {
                if on {
                    p2.set("Settings", key, "true");
                } else {
                    p2.remove("Settings", key);
                }
            }
            match ipv6.selected() {
                1 => p2.set("IPv6", "Enabled", "true"),
                2 => p2.set("IPv6", "Enabled", "false"),
                _ => p2.remove("IPv6", "Enabled"),
            }
            let pw = pass.text().to_string();
            if is_psk && !pw.trim().is_empty() {
                p2.set("Security", "Passphrase", pw.trim());
            }
            if is_8021x {
                p2.set(
                    "Security",
                    "EAP-Method",
                    ["PEAP", "TTLS"][eap_method.selected() as usize],
                );
                for (key, val) in [
                    ("EAP-Identity", eap_id.text().to_string()),
                    ("EAP-Phase2-Identity", eap_p2id.text().to_string()),
                    ("EAP-Phase2-Password", eap_p2pw.text().to_string()),
                    ("EAP-CACert", eap_ca.text().to_string()),
                ] {
                    if val.trim().is_empty() {
                        p2.remove("Security", key);
                    } else {
                        p2.set("Security", key, val.trim());
                    }
                }
                p2.set(
                    "Security",
                    "EAP-Phase2-Method",
                    ["MSCHAPV2", "PAP", "CHAP"][eap_p2m.selected() as usize],
                );
            }
            // DHCP selection must actually DELETE the static keys —
            // merging alone would leave them active.
            for k in ["Address", "Netmask", "Gateway", "DNS"] {
                p2.remove("IPv4", k);
            }
            if method.selected() == 1 {
                for (k, e) in [
                    ("Address", &addr),
                    ("Netmask", &mask),
                    ("Gateway", &gw),
                    ("DNS", &dns),
                ] {
                    let v = e.text().to_string();
                    if !v.trim().is_empty() {
                        p2.set("IPv4", k, v.trim());
                    }
                }
            }
            let (tx, rx) = mpsc::channel();
            let s2 = ssid.clone();
            std::thread::spawn(move || {
                let _ = tx.send(profile_save(&s2, &p2));
            });
            let (w2, u2) = (w.clone(), u.clone());
            gtk4::glib::timeout_add_local(Duration::from_millis(150), move || {
                match rx.try_recv() {
                    Ok(Ok(())) | Err(_) => {
                        w2.close();
                        render(&u2);
                        gtk4::glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("profile save failed: {e:#}");
                        w2.close();
                        gtk4::glib::ControlFlow::Break
                    }
                }
            });
        });
    }
    btns.append(&cancel);
    btns.append(&save);

    // Buttons as the last group on the same page — no reparenting:
    // the page already lives inside the loading view's scroller.
    let btn_group = PreferencesGroup::new();
    let btn_row = ActionRow::new();
    btn_row.add_suffix(&btns);
    btn_group.add(&btn_row);
    page.add(&btn_group);
}
