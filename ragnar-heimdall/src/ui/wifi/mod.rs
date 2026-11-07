//! Wi-Fi section embedded in the Interfaces page: a persistent
//! "Wi-Fi Networks" preferences group that hosts the whole
//! scan → connect workflow. The scan action stays on top, results
//! render below it; secured networks ask for a password, everything
//! is persisted by iwd itself in `/var/lib/iwd`.

mod connect;
mod properties;
mod tasks;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, PreferencesGroup, Spinner};
use gtk4::Image;

use crate::backend::{self, LinkKind, WifiNetwork};

/// Poll interval for worker results arriving on the main loop.
const POLL_MS: u64 = 200;

/// How often the connected state is re-read from iwd — a single
/// `Station.ConnectedNetwork` property fetch. Read-only: it can
/// never drop or alter the association.
const STATUS_SECS: u64 = 4;

/// Shared section state. All fields are interior-mutable because the
/// group outlives every closure that mutates it.
struct Section {
    group: PreferencesGroup,
    iface: String,
    /// Widgets belonging to the current render pass.
    rows: RefCell<Vec<gtk4::Widget>>,
    /// Networks from the last completed scan.
    nets: RefCell<Vec<WifiNetwork>>,
    scanning: Cell<bool>,
    /// SSID currently being connected to, if any.
    connecting: RefCell<Option<String>>,
    /// SSID that is connected right now, if any — synced live from
    /// iwd, not just from our own connect requests.
    connected: RefCell<Option<String>>,
    /// Transient error/info banner shown under the scan row.
    notice: RefCell<Option<String>>,
}

/// Build the Wi-Fi section for the first wireless adapter, if any.
/// Returns `None` on machines without wireless hardware so the
/// Interfaces page simply omits the group.
pub fn section() -> Option<PreferencesGroup> {
    let iface = wifi_iface()?;
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Wi-Fi Networks"))
        .description(format!(
            "{} · {}",
            iface,
            gettextrs::gettext("Select a network to connect.")
        ))
        .build();

    let ui = Rc::new(Section {
        group: group.clone(),
        iface,
        rows: RefCell::new(Vec::new()),
        nets: RefCell::new(Vec::new()),
        scanning: Cell::new(false),
        connecting: RefCell::new(None),
        connected: RefCell::new(None),
        notice: RefCell::new(None),
    });
    // Radio power switch stays pinned at the top of the group — it
    // is not part of the re-rendered row set, so it survives every
    // scan cycle. Bound to the rfkill soft-block state.
    let power = adw::SwitchRow::builder()
        .title(gettextrs::gettext("Wi-Fi radio"))
        .subtitle(gettextrs::gettext("Switches the wireless card on or off."))
        .active(!backend::wlan_soft_blocked())
        .build();
    {
        let power = power.clone();
        power.connect_active_notify(move |sw| {
            let blocked = !sw.is_active();
            std::thread::spawn(move || {
                if let Err(err) = backend::set_wlan_blocked(blocked) {
                    tracing::warn!("rfkill toggle failed: {err:#}");
                }
            });
        });
    }
    group.add(&power);

    // Scan right away — users expect nearby networks to appear
    // without pressing a button, like every other network UI does.
    tasks::start_scan(Rc::clone(&ui));
    // Reflect the live association — an iwd autoconnect or a later
    // drop shows correctly instead of a stale manual flag.
    tasks::sync_connected(Rc::clone(&ui));

    // Periodic live-state sync (read-only). Stops once the page is
    // gone — the WeakRef fails to upgrade.
    let weak = group.downgrade();
    let ui_timer = Rc::clone(&ui);
    gtk4::glib::timeout_add_local(Duration::from_secs(STATUS_SECS), move || {
        if weak.upgrade().is_none() {
            return gtk4::glib::ControlFlow::Break;
        }
        tasks::sync_connected(Rc::clone(&ui_timer));
        gtk4::glib::ControlFlow::Continue
    });

    Some(group)
}

/// First wireless interface name, if the machine has one.
fn wifi_iface() -> Option<String> {
    backend::collect_interfaces()
        .ok()?
        .into_iter()
        .find(|i| i.kind == LinkKind::Wifi)
        .map(|i| i.name)
}

/// Rebuild the section rows from the current state.
fn render(ui: &Rc<Section>) {
    for row in ui.rows.borrow_mut().drain(..) {
        ui.group.remove(&row);
    }

    let busy = ui.scanning.get() || ui.connecting.borrow().is_some();

    // The scan action stays on top so it is always reachable, no
    // matter how long the result list below grows.
    let scan_row = ActionRow::builder()
        .title(if ui.nets.borrow().is_empty() {
            gettextrs::gettext("Scan for Networks")
        } else {
            gettextrs::gettext("Scan again")
        })
        .activatable(!busy)
        .build();
    scan_row.add_prefix(&Image::from_icon_name("view-refresh-symbolic"));
    if !busy {
        let ui2 = Rc::clone(ui);
        scan_row.connect_activated(move |_| tasks::start_scan(Rc::clone(&ui2)));
    }
    push_row(ui, &scan_row.upcast());

    if ui.scanning.get() {
        let row = ActionRow::builder()
            .title(gettextrs::gettext("Scanning…"))
            .build();
        row.add_suffix(&Spinner::new());
        push_row(ui, &row.upcast());
    }

    if let Some(ssid) = ui.connecting.borrow().clone() {
        let row = ActionRow::builder()
            .title(format!("{} {ssid}…", gettextrs::gettext("Connecting to")))
            .build();
        row.add_suffix(&Spinner::new());
        push_row(ui, &row.upcast());
    }

    if let Some(msg) = ui.notice.borrow().clone() {
        let row = ActionRow::builder().subtitle(msg).build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        push_row(ui, &row.upcast());
    }

    // The connected network always pins to the top of the list —
    // it is the one the user most likely wants to act on.
    let connected_ssid = ui.connected.borrow().clone();
    let nets = ui.nets.borrow();
    let mut ordered: Vec<&crate::backend::WifiNetwork> = Vec::new();
    if let Some(ssid) = &connected_ssid {
        ordered.extend(nets.iter().filter(|n| &n.ssid == ssid));
    }
    ordered.extend(
        nets.iter()
            .filter(|n| Some(&n.ssid) != connected_ssid.as_ref()),
    );
    for net in ordered {
        push_row(ui, &network_row(ui, net).upcast());
    }

    // A dedicated disconnect action while associated — the link only
    // goes down on an explicit user request, never implicitly.
    if let Some(ssid) = ui.connected.borrow().clone() {
        let row = ActionRow::builder()
            .title(format!(
                "{} “{ssid}”",
                gettextrs::gettext("Disconnect from")
            ))
            .activatable(true)
            .build();
        row.add_prefix(&Image::from_icon_name("media-playback-stop-symbolic"));
        let ui2 = Rc::clone(ui);
        row.connect_activated(move |_| connect::start_disconnect(Rc::clone(&ui2)));
        push_row(ui, &row.upcast());
    }
}

/// Add a widget to the group and track it as rendered state.
fn push_row(ui: &Rc<Section>, widget: &gtk4::Widget) {
    ui.group.add(widget);
    ui.rows.borrow_mut().push(widget.clone());
}

/// One network row: signal icon, SSID, band + security, and the
/// click target for the connect flow. The currently connected
/// network is not clickable — disconnecting happens only through the
/// dedicated Disconnect row.
fn network_row(ui: &Rc<Section>, net: &WifiNetwork) -> ActionRow {
    let clickable = ui.connecting.borrow().is_none()
        && ui.connected.borrow().as_deref() != Some(net.ssid.as_str());
    let row = ActionRow::builder()
        .title(&net.ssid)
        .subtitle(subtitle(ui, net))
        .activatable(clickable)
        .build();
    row.add_prefix(&Image::from_icon_name(signal_icon(net.signal_dbm)));

    if ui.connected.borrow().as_deref() == Some(net.ssid.as_str()) {
        row.add_suffix(&Image::from_icon_name("emblem-ok-symbolic"));
    } else if net.secured {
        row.add_suffix(&Image::from_icon_name("channel-secure-symbolic"));
    }

    // Stored profiles get a forget button — dropping stale hotel
    // or café networks should not require a terminal.
    if backend::wifi_is_known(&net.ssid) {
        let forget_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text(gettextrs::gettext("Forget saved network"))
            .build();
        forget_btn.add_css_class("flat");
        {
            let ssid = net.ssid.clone();
            let ui2 = Rc::clone(ui);
            forget_btn.connect_clicked(move |_| {
                let ssid = ssid.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(backend::wifi_forget(&ssid));
                });
                let ui3 = Rc::clone(&ui2);
                gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
                    match rx.try_recv() {
                        Ok(result) => {
                            if let Err(err) = result {
                                tracing::warn!("forget failed: {err:#}");
                            }
                            // Re-render so the button disappears.
                            render(&ui3);
                            gtk4::glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
                        Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
                    }
                });
            });
        }
        row.add_suffix(&forget_btn);
    }

    if clickable {
        let ui2 = Rc::clone(ui);
        let net = net.clone();
        row.connect_activated(move |_| connect::begin_connect(Rc::clone(&ui2), net.clone()));
    }

    // Right-click context menu — Connect/Disconnect, Properties,
    // Forget. The full iwd profile editor lives in `properties`.
    {
        let pop = gtk4::Popover::new();
        pop.set_parent(&row);
        pop.set_has_arrow(false);
        let pbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        pbox.set_margin_top(6);
        pbox.set_margin_bottom(6);
        pbox.set_margin_start(6);
        pbox.set_margin_end(6);
        let is_conn = ui.connected.borrow().as_deref() == Some(net.ssid.as_str());
        let conn_btn = gtk4::Button::with_label(&gettextrs::gettext(if is_conn {
            "Disconnect"
        } else {
            "Connect"
        }));
        conn_btn.set_has_frame(false);
        {
            let (u, n, p) = (ui.clone(), net.clone(), pop.clone());
            conn_btn.connect_clicked(move |_| {
                p.popdown();
                if is_conn {
                    connect::start_disconnect(u.clone());
                } else {
                    connect::begin_connect(u.clone(), n.clone());
                }
            });
        }
        pbox.append(&conn_btn);
        let props_btn = gtk4::Button::with_label(&gettextrs::gettext("Properties…"));
        props_btn.set_has_frame(false);
        {
            let (u, n, p) = (ui.clone(), net.clone(), pop.clone());
            props_btn.connect_clicked(move |_| {
                p.popdown();
                crate::ui::wifi::properties::present(&u, n.ssid.clone());
            });
        }
        pbox.append(&props_btn);
        pop.set_child(Some(&pbox));
        let click = gtk4::GestureClick::new();
        click.set_button(3);
        let p = pop.clone();
        click.connect_pressed(move |g, _, x, y| {
            p.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            p.popup();
            g.set_state(gtk4::EventSequenceState::Claimed);
        });
        row.add_controller(click);
    }
    row
}

/// `"72 % · 5 GHz · Secured"` from dBm and frequency; the connected
/// network is marked accordingly.
fn subtitle(ui: &Rc<Section>, net: &WifiNetwork) -> String {
    let band = if net.frequency_mhz > 3000 {
        "5 GHz"
    } else {
        "2.4 GHz"
    };
    let secured = if net.secured {
        gettextrs::gettext("Secured")
    } else {
        gettextrs::gettext("Open")
    };
    let text = format!("{} % · {band} · {secured}", signal_percent(net.signal_dbm));
    if ui.connected.borrow().as_deref() == Some(net.ssid.as_str()) {
        format!("{} · {text}", gettextrs::gettext("Connected"))
    } else {
        text
    }
}

/// Rough dBm → percentage mapping used for display only.
fn signal_percent(dbm: i16) -> i16 {
    (2 * (dbm + 100)).clamp(0, 100)
}

/// Signal-strength icon name per dBm bucket.
fn signal_icon(dbm: i16) -> &'static str {
    if dbm >= -50 {
        "network-wireless-signal-excellent-symbolic"
    } else if dbm >= -60 {
        "network-wireless-signal-good-symbolic"
    } else if dbm >= -70 {
        "network-wireless-signal-ok-symbolic"
    } else {
        "network-wireless-signal-weak-symbolic"
    }
}
