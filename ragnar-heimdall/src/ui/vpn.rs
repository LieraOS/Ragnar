//! VPN page: import WireGuard/OpenVPN profiles, toggle tunnels and
//! autostart, delete profiles. Reading profile state needs the
//! privileged helper (configs live in root-only dirs), so — like
//! the firewall page — the first load is deferred until the page is
//! actually selected to avoid a surprise polkit prompt at startup.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{
    ActionRow, AlertDialog, ExpanderRow, PreferencesGroup, PreferencesPage, Spinner, SwitchRow,
};
use gtk4::Image;

use crate::backend::{self, VpnKind, VpnProfile};

/// Poll interval for worker results arriving on the main loop.
const POLL_MS: u64 = 200;

/// Page state shared across async refresh/mutation callbacks.
struct Vpn {
    page: PreferencesPage,
    groups: RefCell<Vec<PreferencesGroup>>,
    profiles: RefCell<Vec<VpnProfile>>,
    busy: Cell<bool>,
    notice: RefCell<Option<String>>,
}

/// Build the VPN page. The privileged profile read is deferred to
/// the first visit — same pattern as the firewall page.
pub fn build(stack: &gtk4::Stack) -> gtk4::Widget {
    let page = PreferencesPage::new();
    let vpn = Rc::new(Vpn {
        page: page.clone(),
        groups: RefCell::new(Vec::new()),
        profiles: RefCell::new(Vec::new()),
        busy: Cell::new(false),
        notice: RefCell::new(None),
    });

    if !backend::wg_available() && !backend::ovpn_available() {
        let group = PreferencesGroup::builder()
            .title(gettextrs::gettext("VPN"))
            .build();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("No VPN backend installed"))
            .subtitle(gettextrs::gettext(
                "Install wireguard-tools or openvpn to manage VPN connections.",
            ))
            .build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        page.add(&group);
        return page.upcast();
    }

    // Import group is always visible — it works regardless of state.
    add_group(&vpn, &import_group(&vpn));

    let loaded = Rc::new(Cell::new(false));
    let on_shown: Rc<dyn Fn()> = {
        let vpn = Rc::clone(&vpn);
        let loaded = Rc::clone(&loaded);
        Rc::new(move || {
            if !loaded.replace(true) {
                refresh(Rc::clone(&vpn));
            }
        })
    };
    stack.connect_notify_local(Some("visible-child-name"), {
        let on_shown = Rc::clone(&on_shown);
        move |stack, _| {
            if stack.visible_child_name().as_deref() == Some(super::sidebar::View::Vpn.tag()) {
                on_shown();
            }
        }
    });
    if stack.visible_child_name().as_deref() == Some(super::sidebar::View::Vpn.tag()) {
        on_shown();
    }
    page.upcast()
}

/// Group with the two import actions — one per supported kind, so
/// `.conf` files never have to be guessed.
fn import_group(vpn: &Rc<Vpn>) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("VPN profiles"))
        .description(gettextrs::gettext(
            "Import the config file your VPN provider gives you — WireGuard \
             (.conf) or OpenVPN (.ovpn/.conf).",
        ))
        .build();
    for (kind, title) in [
        (
            VpnKind::WireGuard,
            gettextrs::gettext("Import WireGuard profile…"),
        ),
        (
            VpnKind::OpenVpn,
            gettextrs::gettext("Import OpenVPN profile…"),
        ),
    ] {
        let available = match kind {
            VpnKind::WireGuard => backend::wg_available(),
            VpnKind::OpenVpn => backend::ovpn_available(),
        };
        let row = ActionRow::builder()
            .title(title)
            .activatable(available)
            .sensitive(available)
            .build();
        row.add_prefix(&Image::from_icon_name(match kind {
            VpnKind::WireGuard => "network-vpn-symbolic",
            VpnKind::OpenVpn => "network-vpn-symbolic",
        }));
        if available {
            let vpn = Rc::clone(vpn);
            row.connect_activated(move |r| {
                let vpn = Rc::clone(&vpn);
                super::vpn_add::present(r, kind, move |name, path| {
                    let vpn2 = Rc::clone(&vpn);
                    mutate(vpn2, move || backend::vpn_import(kind, &name, &path));
                });
            });
        }
        group.add(&row);
    }
    group
}

/// (Re)load the profile inventory in a worker, then re-render.
fn refresh(vpn: Rc<Vpn>) {
    vpn.busy.set(true);
    render(&vpn);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(backend::vpn_profiles());
    });
    let vpn2 = Rc::clone(&vpn);
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                vpn2.busy.set(false);
                match result {
                    Ok(list) => {
                        *vpn2.profiles.borrow_mut() = list;
                        *vpn2.notice.borrow_mut() = None;
                    }
                    Err(err) => {
                        tracing::warn!("vpn status failed: {err:#}");
                        *vpn2.notice.borrow_mut() = Some(format!("{err:#}"));
                    }
                }
                render(&vpn2);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Run a mutating helper call in a worker, then refresh.
fn mutate<F: FnOnce() -> anyhow::Result<()> + Send + 'static>(vpn: Rc<Vpn>, op: F) {
    vpn.busy.set(true);
    render(&vpn);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = op();
        let _ = tx.send(result);
    });
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                if let Err(err) = result {
                    tracing::warn!("vpn operation failed: {err:#}");
                    *vpn.notice.borrow_mut() = Some(format!("{err:#}"));
                    vpn.busy.set(false);
                    render(&vpn);
                } else {
                    refresh(vpn.clone());
                }
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Rebuild data groups — the import group (index 0) survives.
fn render(vpn: &Rc<Vpn>) {
    for group in vpn.groups.borrow_mut().drain(1..) {
        vpn.page.remove(&group);
    }

    if let Some(msg) = vpn.notice.borrow().clone() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder().subtitle(msg).build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        add_group(vpn, &group);
    }

    if vpn.busy.get() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("Working…"))
            .build();
        row.add_suffix(&Spinner::new());
        group.add(&row);
        add_group(vpn, &group);
        return;
    }

    let profiles = vpn.profiles.borrow().clone();
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Connections"))
        .build();
    if profiles.is_empty() {
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("No VPN profiles yet"))
                .subtitle(gettextrs::gettext(
                    "Import a config file above to get started.",
                ))
                .build(),
        );
    }
    for profile in &profiles {
        group.add(&profile_row(vpn, profile));
    }
    add_group(vpn, &group);
}

/// Track a freshly added group for the next render pass.
fn add_group(vpn: &Rc<Vpn>, group: &PreferencesGroup) {
    vpn.page.add(group);
    vpn.groups.borrow_mut().push(group.clone());
}

/// One profile: expander with kind/state summary; inside, the
/// connect switch, autostart switch, transfer stats and delete.
fn profile_row(vpn: &Rc<Vpn>, profile: &VpnProfile) -> ExpanderRow {
    let mut subtitle = format!("{} · ", profile.kind.label());
    subtitle.push_str(&if profile.active {
        gettextrs::gettext("connected")
    } else {
        gettextrs::gettext("disconnected")
    });
    if profile.autostart {
        subtitle.push_str(&format!(" · {}", gettextrs::gettext("starts at boot")));
    }

    let row = ExpanderRow::builder()
        .title(&profile.name)
        .subtitle(&subtitle)
        .build();
    row.add_prefix(&Image::from_icon_name("network-vpn-symbolic"));

    // Connect switch.
    let conn = SwitchRow::builder()
        .title(gettextrs::gettext("Connect"))
        .active(profile.active)
        .build();
    {
        let vpn = Rc::clone(vpn);
        let profile = profile.clone();
        conn.connect_active_notify(move |sw| {
            let p = profile.clone();
            let up = sw.is_active();
            let vpn2 = Rc::clone(&vpn);
            mutate(vpn2, move || backend::vpn_set_up(&p, up));
        });
    }
    row.add_row(&conn);

    // Autostart switch — the matching systemd unit controls boot.
    let auto = SwitchRow::builder()
        .title(gettextrs::gettext("Connect automatically at boot"))
        .active(profile.autostart)
        .build();
    {
        let vpn = Rc::clone(vpn);
        let profile = profile.clone();
        auto.connect_active_notify(move |sw| {
            let p = profile.clone();
            let on = sw.is_active();
            let vpn2 = Rc::clone(&vpn);
            mutate(vpn2, move || backend::vpn_set_autostart(&p, on));
        });
    }
    row.add_row(&auto);

    // Transfer stats when the tunnel is up (WireGuard reports them).
    if profile.active {
        if let (Some(rx), Some(tx)) = (profile.rx_bytes, profile.tx_bytes) {
            row.add_row(
                &ActionRow::builder()
                    .title(gettextrs::gettext("Transferred"))
                    .subtitle(format!("↓ {} · ↑ {}", human_bytes(rx), human_bytes(tx)))
                    .build(),
            );
        }
    }

    let del = ActionRow::builder()
        .title(gettextrs::gettext("Delete profile"))
        .activatable(true)
        .build();
    del.add_prefix(&Image::from_icon_name("user-trash-symbolic"));
    {
        let vpn = Rc::clone(vpn);
        let profile = profile.clone();
        del.connect_activated(move |r| {
            let vpn2 = Rc::clone(&vpn);
            let p_dialog = profile.clone();
            let p_delete = profile.clone();
            confirm_delete(r, &p_dialog, move || {
                let p = p_delete.clone();
                mutate(vpn2.clone(), move || backend::vpn_delete(&p));
            });
        });
    }
    row.add_row(&del);
    row
}

/// Confirm before wiping a profile — the config file is gone for
/// good, the provider would have to issue a new one.
fn confirm_delete<F: Fn() + 'static>(source: &ActionRow, profile: &VpnProfile, on_yes: F) {
    let dialog = AlertDialog::builder()
        .heading(gettextrs::gettext("Delete VPN profile?"))
        .body(format!(
            "{} “{}”. {}",
            gettextrs::gettext("This removes"),
            profile.name,
            gettextrs::gettext("The config file is deleted permanently.")
        ))
        .build();
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("delete", &gettextrs::gettext("Delete"));
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            on_yes();
        }
    });
    dialog.present(Some(source));
}

/// Byte counts in short human form — 1048576 → "1.0 MiB".
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
