// Copyright (C) Jörg Schröder
//! Virtual interface creation dialog — VLAN, bridge and bond in one
//! resizable window. Fields that only apply to a kind are hidden for
//! the others; bond members are enslaved right after creation.

use libadwaita as adw;

use adw::prelude::*;

use crate::backend::{self, BOND_MODES};

/// Present the create dialog. `parents` are candidate VLAN parents,
/// `members` physical links that may join a bridge/bond.
pub fn create_dialog(parent: &impl IsA<gtk4::Widget>, parents: Vec<String>, members: Vec<String>) {
    let win = adw::Window::builder()
        .title(gettextrs::gettext("Virtual interface"))
        .default_width(560)
        .default_height(560)
        .modal(true)
        .build();
    // Transient-for needs the toplevel window; absent one the dialog
    // simply presents non-modal-free rather than panicking.
    if let Some(w) = parent.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&w));
    }

    let kind = adw::ComboRow::builder()
        .title(gettextrs::gettext("Type"))
        .model(&gtk4::StringList::new(&["VLAN", "Bridge", "Bond"]))
        .build();
    let name = adw::EntryRow::builder()
        .title(gettextrs::gettext("Name (e.g. vlan10, br0, bond0)"))
        .build();
    let parent_row = adw::ComboRow::builder()
        .title(gettextrs::gettext("Parent interface"))
        .model(&gtk4::StringList::new(
            &parents.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .build();
    let vlan_id = adw::EntryRow::builder()
        .title(gettextrs::gettext("VLAN ID (1–4094)"))
        .build();
    let bond_mode = adw::ComboRow::builder()
        .title(gettextrs::gettext("Bond mode"))
        .model(&gtk4::StringList::new(BOND_MODES))
        .build();

    // Member check rows for bridge/bond — enslaved after creation.
    // CheckButtons are kept in a direct list; digging them back out
    // of the row's internal suffix box would be fragile.
    let member_box = gtk4::ListBox::new();
    member_box.add_css_class("boxed-list");
    member_box.set_selection_mode(gtk4::SelectionMode::None);
    let member_widgets: Vec<(String, gtk4::CheckButton)> = members
        .iter()
        .map(|m| {
            let check = gtk4::CheckButton::new();
            let row = adw::ActionRow::builder().title(m).activatable(true).build();
            row.add_suffix(&check);
            row.connect_activated({
                let check = check.clone();
                move |_| check.set_active(!check.is_active())
            });
            member_box.append(&row);
            (m.clone(), check)
        })
        .collect();

    let member_group = adw::PreferencesGroup::builder()
        .title(gettextrs::gettext("Member interfaces"))
        .build();
    member_group.add(&member_box);

    let basic = adw::PreferencesGroup::new();
    basic.add(&kind);
    basic.add(&name);
    basic.add(&parent_row);
    basic.add(&vlan_id);
    basic.add(&bond_mode);

    let page = adw::PreferencesPage::new();
    page.add(&basic);
    page.add(&member_group);

    // Buttons sit in their own group at the bottom of the page.
    let cancel = gtk4::Button::with_label(&gettextrs::gettext("Cancel"));
    let ok = gtk4::Button::with_label(&gettextrs::gettext("Create"));
    ok.add_css_class("suggested-action");
    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    btn_box.set_halign(gtk4::Align::End);
    btn_box.append(&cancel);
    btn_box.append(&ok);
    let btn_row = adw::ActionRow::new();
    btn_row.set_child(Some(&btn_box));
    let btn_group = adw::PreferencesGroup::new();
    btn_group.add(&btn_row);
    page.add(&btn_group);

    // Kind-dependent visibility: parent+VLAN id only for VLAN, bond
    // mode only for bonds, member list only for bridge/bond.
    let sync_visibility = {
        let parent_row = parent_row.clone();
        let vlan_id = vlan_id.clone();
        let bond_mode = bond_mode.clone();
        let member_group = member_group.clone();
        move |sel: u32| {
            parent_row.set_visible(sel == 0);
            vlan_id.set_visible(sel == 0);
            bond_mode.set_visible(sel == 2);
            member_group.set_visible(sel != 0);
        }
    };
    sync_visibility(kind.selected());
    kind.connect_selected_notify(move |k| sync_visibility(k.selected()));

    // Without vexpand the scroll area only gets its minimal height
    // and the whole form collapses into a tiny scroll slit.
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&page)
        .vexpand(true)
        .hexpand(true)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .min_content_height(480)
        .build();
    let bar = adw::HeaderBar::new();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&bar);
    content.append(&scroll);
    win.set_content(Some(&content));

    cancel.connect_clicked({
        let win = win.clone();
        move |_| win.close()
    });
    ok.connect_clicked({
        let win = win.clone();
        move |_| {
            let sel = kind.selected();
            let kind_str = ["vlan", "bridge", "bond"][sel as usize];
            let n = name.text().trim().to_string();
            let p = parents
                .get(parent_row.selected() as usize)
                .cloned()
                .unwrap_or_default();
            let vid = vlan_id.text().trim().parse::<u32>().unwrap_or(0);
            let mode = BOND_MODES
                .get(bond_mode.selected() as usize)
                .copied()
                .unwrap_or(BOND_MODES[0])
                .to_string();
            let enslaved: Vec<String> = member_widgets
                .iter()
                .filter(|(_, c)| c.is_active())
                .map(|(m, _)| m.clone())
                .collect();
            // GTK widgets are !Send — the worker reports back through
            // a channel and the main loop closes the window.
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            std::thread::spawn(move || {
                let r = backend::add_virtual(&n, kind_str, &p, vid, &mode);
                if let Err(err) = &r {
                    tracing::warn!("create virtual interface failed: {err:#}");
                    let _ = tx.send(false);
                    return;
                }
                for m in enslaved {
                    if let Err(err) = backend::set_master(&m, Some(&n)) {
                        tracing::warn!("enslave {m} to {n} failed: {err:#}");
                    }
                }
                let _ = tx.send(true);
            });
            gtk4::glib::timeout_add_local(std::time::Duration::from_millis(200), {
                let win = win.clone();
                move || match rx.try_recv() {
                    Ok(true) => {
                        win.close();
                        gtk4::glib::ControlFlow::Break
                    }
                    Ok(false) => gtk4::glib::ControlFlow::Break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        gtk4::glib::ControlFlow::Break
                    }
                }
            });
        }
    });
    win.present();
}

/// Confirmation + delete for one virtual interface row.
pub fn confirm_delete(source: &impl IsA<gtk4::Widget>, name: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(gettextrs::gettext("Remove virtual interface?"))
        .body(format!(
            "{} “{name}” — {}",
            gettextrs::gettext("This deletes"),
            gettextrs::gettext("attached member links are released.")
        ))
        .build();
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("delete", &gettextrs::gettext("Delete"));
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");
    let name = name.to_string();
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            let name = name.clone();
            std::thread::spawn(move || {
                if let Err(err) = backend::del_virtual(&name) {
                    tracing::warn!("delete {name} failed: {err:#}");
                }
            });
        }
    });
    dialog.present(Some(source));
}
