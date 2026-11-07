//! Interfaces view: every kernel network interface as an expandable
//! row with its live state, MAC, MTU and addresses. Rows refresh
//! periodically from a fresh `ip -json` snapshot — strictly read-only,
//! so a refresh can never drop or alter a connection.
//!
//! The enable switch mirrors the *administrative* link state
//! (IFF_UP), not the carrier — an unplugged cable is shown as
//! "waiting for cable" but keeps the interface switched on.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, ExpanderRow, PreferencesGroup, PreferencesPage};

use crate::backend::{self, LinkKind, LinkState, NetworkInterface};

use super::{ipv4_add, iwd_opts, preference, vlink_add};

/// Interval between read-only interface snapshots.
const REFRESH_SECS: u64 = 3;

/// Route-metric presets for the priority combo — lower wins.
const METRIC_PREFERRED: u32 = 100;
const METRIC_FALLBACK: u32 = 600;

/// Widgets for one interface row. The expander itself survives every
/// refresh so a user-opened row stays open; only its subtitle and
/// detail rows are rewritten.
struct IfaceRow {
    expander: ExpanderRow,
    details: Vec<gtk4::Widget>,
}

/// Build the interfaces page from a fresh kernel snapshot.
pub fn build() -> gtk4::Widget {
    let page = PreferencesPage::new();
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Network Interfaces"))
        .build();

    let rows: Rc<RefCell<HashMap<String, IfaceRow>>> = Rc::new(RefCell::new(HashMap::new()));
    let warn_row: Rc<RefCell<Option<gtk4::Widget>>> = Rc::new(RefCell::new(None));
    // Latest snapshot for the create dialog's parent/member pickers
    // and the connection-preference combo.
    let latest: Rc<RefCell<Vec<NetworkInterface>>> = Rc::new(RefCell::new(Vec::new()));

    if let Err(err) = refresh(&group, &rows, &warn_row, &latest) {
        tracing::warn!("interface collection failed: {err:#}");
        let warn = ActionRow::builder()
            .title(gettextrs::gettext("Could not read interface state"))
            .build();
        warn.add_prefix(&gtk4::Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&warn);
        *warn_row.borrow_mut() = Some(warn.upcast());
    }

    // Global connection preference sits on top — the coarse dial a
    // normal user reaches for first; per-link metrics stay below.
    page.add(&preference::group(&latest));
    page.add(&group);

    // Advanced group: virtual interface creation for power users.
    let advanced = PreferencesGroup::builder()
        .title(gettextrs::gettext("Advanced"))
        .description(gettextrs::gettext(
            "Create VLANs, bridges and bonds for advanced setups.",
        ))
        .build();
    let create = ActionRow::builder()
        .title(gettextrs::gettext("Create virtual interface…"))
        .activatable(true)
        .build();
    create.add_prefix(&gtk4::Image::from_icon_name("list-add-symbolic"));
    {
        let latest = Rc::clone(&latest);
        create.connect_activated(move |row| {
            let ifaces = latest.borrow();
            let parents: Vec<String> = ifaces
                .iter()
                .filter(|i| matches!(i.kind, LinkKind::Ethernet | LinkKind::Wifi))
                .map(|i| i.name.clone())
                .collect();
            vlink_add::create_dialog(row, parents.clone(), parents);
        });
    }
    advanced.add(&create);
    page.add(&advanced);

    // Wireless scanning lives directly on this page as its own
    // clearly-labelled group — adapter list and Wi-Fi workflow stay
    // together instead of a separate page. The iwd options group
    // joins it: privacy-related radio settings belong where the
    // radio is managed.
    if let Some(wifi) = super::wifi::section() {
        page.add(&wifi);
        page.add(&iwd_opts::wifi_options());
    }

    // Periodic read-only refresh: re-runs the `ip` snapshot and
    // reconciles rows in place. The timer stops itself once the page
    // is gone — the WeakRef fails to upgrade.
    let weak = group.downgrade();
    gtk4::glib::timeout_add_local(Duration::from_secs(REFRESH_SECS), move || {
        let Some(group) = weak.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };
        let _ = refresh(&group, &rows, &warn_row, &latest);
        gtk4::glib::ControlFlow::Continue
    });

    page.upcast()
}

/// Re-collect and reconcile rows against the live snapshot: update
/// existing rows in place, create missing ones, drop vanished ones.
/// A successful run also clears a stale warning row.
fn refresh(
    group: &PreferencesGroup,
    rows: &Rc<RefCell<HashMap<String, IfaceRow>>>,
    warn_row: &Rc<RefCell<Option<gtk4::Widget>>>,
    latest: &Rc<RefCell<Vec<NetworkInterface>>>,
) -> anyhow::Result<()> {
    let interfaces = backend::collect_interfaces()?;
    *latest.borrow_mut() = interfaces.clone();

    if let Some(w) = warn_row.borrow_mut().take() {
        group.remove(&w);
    }

    let mut alive = HashSet::with_capacity(interfaces.len());
    for iface in &interfaces {
        alive.insert(iface.name.clone());
        if !rows.borrow().contains_key(&iface.name) {
            let row = make_row(iface);
            group.add(&row.expander);
            rows.borrow_mut().insert(iface.name.clone(), row);
        }
        if let Some(r) = rows.borrow_mut().get_mut(&iface.name) {
            update_row(r, iface);
        }
    }

    // Interfaces the kernel no longer reports are removed.
    rows.borrow_mut().retain(|name, r| {
        let keep = alive.contains(name);
        if !keep {
            group.remove(&r.expander);
        }
        keep
    });
    Ok(())
}

/// Create the expander row for one interface and fill it once.
fn make_row(iface: &NetworkInterface) -> IfaceRow {
    let expander = ExpanderRow::builder().title(&iface.name).build();
    expander.add_prefix(&gtk4::Image::from_icon_name(kind_icon(iface.kind)));
    let mut row = IfaceRow {
        expander,
        details: Vec::new(),
    };
    update_row(&mut row, iface);
    row
}

/// Append a detail row to the expander and track it for rewrites.
fn push_detail(row: &mut IfaceRow, w: gtk4::Widget) {
    row.expander.add_row(&w);
    row.details.push(w);
}

/// Rewrite a row's subtitle and detail rows from a fresh snapshot —
/// the expander's open/closed state lives on the widget itself and
/// is untouched.
fn update_row(row: &mut IfaceRow, iface: &NetworkInterface) {
    row.expander.set_subtitle(&summary(iface));
    for d in row.details.drain(..) {
        row.expander.remove(&d);
    }
    if let Some(mac) = &iface.mac {
        let w = detail(gettextrs::gettext("MAC Address"), mac).upcast::<gtk4::Widget>();
        push_detail(row, w);
    }
    if let Some(mtu) = iface.mtu {
        let w = detail(gettextrs::gettext("MTU"), &mtu.to_string()).upcast::<gtk4::Widget>();
        push_detail(row, w);
    }
    for addr in &iface.addresses {
        let w = detail(gettextrs::gettext("Address"), addr).upcast::<gtk4::Widget>();
        push_detail(row, w);
    }
    if let Some(d) = &iface.driver {
        let w = detail(gettextrs::gettext("Driver"), d).upcast::<gtk4::Widget>();
        push_detail(row, w);
    }
    if let Some(sp) = iface.speed_mbps {
        let w = detail(gettextrs::gettext("Link speed"), &format!("{sp} Mb/s"))
            .upcast::<gtk4::Widget>();
        push_detail(row, w);
    }
    if let (Some(rx), Some(tx)) = (iface.rx_bytes, iface.tx_bytes) {
        let mb = |b: u64| format!("{:.1} MB", b as f64 / 1e6);
        let w = detail(
            gettextrs::gettext("Received / sent"),
            &format!("{} / {}", mb(rx), mb(tx)),
        )
        .upcast::<gtk4::Widget>();
        push_detail(row, w);
    }

    // Loopback stays informational only — every real link gets the
    // controls below.
    if matches!(iface.kind, LinkKind::Loopback) {
        return;
    }

    push_detail(row, link_switch(iface));
    push_detail(row, ipv6_switch(iface));
    push_detail(row, metric_combo(iface));

    let cfg = ActionRow::builder()
        .title(gettextrs::gettext("Configure IPv4…"))
        .activatable(true)
        .build();
    cfg.add_prefix(&gtk4::Image::from_icon_name("document-edit-symbolic"));
    {
        let name = iface.name.clone();
        cfg.connect_activated(move |r| ipv4_add::ipv4_dialog(r, &name));
    }
    push_detail(row, cfg.upcast());

    // Virtual links (VLAN/bridge/bond) get a delete affordance —
    // physical NICs cannot be deleted by the kernel anyway.
    if matches!(
        iface.kind,
        LinkKind::Vlan | LinkKind::Bridge | LinkKind::Bond
    ) {
        let del = ActionRow::builder()
            .title(gettextrs::gettext("Remove virtual interface"))
            .activatable(true)
            .build();
        del.add_prefix(&gtk4::Image::from_icon_name("user-trash-symbolic"));
        {
            let name = iface.name.clone();
            del.connect_activated(move |r| vlink_add::confirm_delete(r, &name));
        }
        push_detail(row, del.upcast());
    }
}

/// Administrative link switch: `active` mirrors IFF_UP so an
/// unplugged cable does not read as "off" — the carrier state is
/// shown as a hint instead.
fn link_switch(iface: &NetworkInterface) -> gtk4::Widget {
    let sw = adw::SwitchRow::builder()
        .title(gettextrs::gettext("Interface enabled"))
        .subtitle(if iface.admin_up && !iface.carrier {
            gettextrs::gettext("Enabled — waiting for cable")
        } else {
            String::new()
        })
        .active(iface.admin_up)
        .build();
    let name = iface.name.clone();
    sw.connect_active_notify(move |sw| {
        let name = name.clone();
        let up = sw.is_active();
        std::thread::spawn(move || {
            if let Err(err) = backend::set_link(&name, up) {
                tracing::warn!("set link failed: {err:#}");
            }
        });
    });
    sw.upcast()
}

/// IPv6 kernel toggle mirroring the real `disable_ipv6` sysctl —
/// persisted by the helper via sysctl.d.
fn ipv6_switch(iface: &NetworkInterface) -> gtk4::Widget {
    let sw = adw::SwitchRow::builder()
        .title(gettextrs::gettext("IPv6 enabled"))
        .active(!iface.ipv6_disabled)
        .build();
    let name = iface.name.clone();
    sw.connect_active_notify(move |sw| {
        let name = name.clone();
        let on = sw.is_active();
        std::thread::spawn(move || {
            if let Err(err) = backend::set_ipv6(&name, on) {
                tracing::warn!("set ipv6 failed: {err:#}");
            }
        });
    });
    sw.upcast()
}

/// Route priority combo: automatic (no override), preferred (low
/// metric — wins over other links), fallback (high metric — only
/// used when the preferred link is gone).
fn metric_combo(iface: &NetworkInterface) -> gtk4::Widget {
    let labels = [
        gettextrs::gettext("Automatic"),
        gettextrs::gettext("Preferred"),
        gettextrs::gettext("Fallback"),
    ];
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let combo = adw::ComboRow::builder()
        .title(gettextrs::gettext("Route priority"))
        .model(&gtk4::StringList::new(&refs))
        .selected(match iface.route_metric {
            None => 0,
            Some(m) if m <= METRIC_PREFERRED => 1,
            Some(_) => 2,
        })
        .build();
    let name = iface.name.clone();
    combo.connect_selected_notify(move |c| {
        let metric = match c.selected() {
            1 => Some(METRIC_PREFERRED),
            2 => Some(METRIC_FALLBACK),
            _ => None,
        };
        let name = name.clone();
        std::thread::spawn(move || {
            if let Err(err) = backend::set_route_metric(&name, metric) {
                tracing::warn!("set route metric failed: {err:#}");
            }
        });
    });
    combo.upcast()
}

/// `"Ethernet · Up"`-style summary for the row subtitle.
fn summary(iface: &NetworkInterface) -> String {
    let state = match iface.state {
        LinkState::Up => gettextrs::gettext("Up"),
        LinkState::Down => gettextrs::gettext("Down"),
        LinkState::Unknown => gettextrs::gettext("Unknown"),
    };
    format!("{} · {state}", kind_label(iface.kind))
}

/// Read-only detail row.
fn detail(title: String, value: &str) -> ActionRow {
    ActionRow::builder().title(title).subtitle(value).build()
}

/// Freedesktop icon per link kind.
fn kind_icon(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Wifi => "network-wireless-symbolic",
        LinkKind::Ethernet => "network-wired-symbolic",
        LinkKind::Loopback => "network-workgroup-symbolic",
        LinkKind::Bridge => "network-server-symbolic",
        LinkKind::Bond => "network-workgroup-symbolic",
        LinkKind::Vlan => "network-wired-symbolic",
        LinkKind::Wireguard => "network-vpn-symbolic",
        LinkKind::Tunnel => "network-vpn-symbolic",
        LinkKind::Other => "network-wired-symbolic",
    }
}

/// Translated link kind label.
fn kind_label(kind: LinkKind) -> String {
    let text = match kind {
        LinkKind::Wifi => "Wi-Fi",
        LinkKind::Ethernet => "Ethernet",
        LinkKind::Loopback => "Loopback",
        LinkKind::Bridge => "Bridge",
        LinkKind::Bond => "Bond",
        LinkKind::Vlan => "VLAN",
        LinkKind::Wireguard => "WireGuard",
        LinkKind::Tunnel => "Tunnel",
        LinkKind::Other => "Other",
    };
    gettextrs::gettext(text)
}
