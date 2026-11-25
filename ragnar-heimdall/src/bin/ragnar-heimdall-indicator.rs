// Copyright (C) Jörg Schröder
//! Panel status indicator for the Ragnar desktop — the replacement
//! for the network icon `nm-applet` used to provide before the stack
//! moved to iwd + networkd. Registers a StatusNotifierItem the
//! Ragnar panel's status applet picks up, shows the live link state
//! (wireless + SSID, wired, or offline), and opens Heimdall on
//! click. All state reads are passive: the indicator can never
//! drop or alter a connection.

use std::process::Command;
use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{MenuItem, ToolTip};

use ragnar_heimdall::backend::{self, LinkKind};
use ragnar_heimdall::i18n;

/// Interval between passive state probes (`ip` snapshot + one iwd
/// property read).
const POLL_SECS: u64 = 3;

/// The indicator's mutable display state — rewritten by the poller.
struct NetIndicator {
    /// Freedesktop icon name for the current link state.
    icon: String,
    /// Tooltip/subtitle text (SSID, "Wired", "Offline", ...).
    text: String,
    /// Wireless iface name while it carries the connection — the
    /// disconnect menu entry needs it.
    wifi: Option<String>,
    /// Associated SSID while connected.
    ssid: Option<String>,
}

impl ksni::Tray for NetIndicator {
    fn id(&self) -> String {
        "org.lieraos.ragnar.Heimdall".into()
    }

    fn title(&self) -> String {
        gettextrs::gettext("Network")
    }

    fn icon_name(&self) -> String {
        self.icon.clone()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: self.title(),
            description: self.text.clone(),
            icon_name: self.icon.clone(),
            ..Default::default()
        }
    }

    /// Left click opens the control center — same gesture nm-applet
    /// users expect.
    fn activate(&mut self, _x: i32, _y: i32) {
        open_ragnar_heimdall();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![StandardItem {
            label: gettextrs::gettext("Open Heimdall"),
            icon_name: "network-workgroup".into(),
            activate: Box::new(|_| open_ragnar_heimdall()),
            ..Default::default()
        }
        .into()];
        // Disconnect is offered only while actually associated —
        // and only ever on explicit user request.
        if let Some(ssid) = &self.ssid {
            items.push(
                StandardItem {
                    label: format!("{} “{ssid}”", gettextrs::gettext("Disconnect from")),
                    icon_name: "network-offline-symbolic".into(),
                    activate: Box::new(|this: &mut NetIndicator| {
                        if let Some(iface) = &this.wifi {
                            if let Err(err) = backend::wifi_disconnect(iface) {
                                tracing::warn!("indicator disconnect failed: {err:#}");
                            }
                        }
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        items
    }
}

/// Well-known bus name used as the single-instance lock. The SNI
/// spec assigns every process its own unique name, so it cannot
/// deduplicate for us — but a named lock can. Held for the whole
/// process lifetime; released automatically when the process dies,
/// which is exactly the crash-safe behaviour we need.
const LOCK_NAME: &str = "org.lieraos.ragnar.Heimdall.indicator";

/// Try to become the sole running indicator instance. Returns the
/// session-bus connection (must stay alive to keep the lock) or
/// None when another instance already owns the name.
fn acquire_single_instance() -> Option<zbus::blocking::Connection> {
    let conn = zbus::blocking::Connection::session().ok()?;
    // request_name errors (NameTaken) when another instance already
    // owns the lock — a second instance would only add a duplicate
    // tray icon anyway.
    conn.request_name(LOCK_NAME).ok()?;
    Some(conn)
}

/// Launch the Heimdall control center detached from this process.
fn open_ragnar_heimdall() {
    if let Err(err) = Command::new("ragnar-heimdall").spawn() {
        tracing::warn!("could not launch ragnar-heimdall: {err}");
    }
}

/// Interface carrying the lowest-metric default route — that is the
/// link user traffic actually takes, so it is what the icon shows.
/// An associated-but-fallback Wi-Fi must not claim the icon while a
/// preferred wired link routes everything.
fn default_route_iface() -> Option<String> {
    let out = Command::new("ip")
        .args(["-j", "route", "show", "default"])
        .output()
        .ok()?;
    let routes: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).ok()?;
    routes
        .iter()
        .filter_map(|r| {
            let dev = r["dev"].as_str()?;
            let metric = r["metric"].as_u64().unwrap_or(u64::MAX);
            Some((metric, dev))
        })
        .min_by_key(|(m, _)| *m)
        .map(|(_, dev)| dev.to_string())
}

/// Passive snapshot of the link state: the icon follows the default
/// route (wired or wireless), falling back to association state when
/// no route exists yet — DHCP may still be negotiating.
fn probe() -> (String, String, Option<String>, Option<String>) {
    let Ok(ifaces) = backend::collect_interfaces() else {
        return (
            "network-offline-symbolic".into(),
            gettextrs::gettext("Unknown"),
            None,
            None,
        );
    };

    // Which interface owns the active default route, if any.
    let routed = default_route_iface();
    let routed_wifi = routed
        .as_deref()
        .and_then(|dev| ifaces.iter().find(|i| i.name == dev))
        .map(|i| i.kind == LinkKind::Wifi)
        .unwrap_or(false);

    // Wireless icon only when Wi-Fi actually carries the route —
    // or when it is the sole associated link with no route yet.
    if let Some(w) = ifaces.iter().find(|i| i.kind == LinkKind::Wifi) {
        if let Some(ssid) = backend::wifi_connected_ssid(&w.name) {
            let owns_route = routed.as_deref() == Some(w.name.as_str());
            let no_route_for_other = routed.is_none();
            if owns_route || no_route_for_other {
                let text = if w.has_usable_ipv4() {
                    ssid.clone()
                } else {
                    format!("{ssid} ({})", gettextrs::gettext("no IP"))
                };
                return (
                    "network-wireless-symbolic".into(),
                    text,
                    Some(w.name.clone()),
                    Some(ssid),
                );
            }
        }
    }

    // Wired icon whenever a wired link carries the route or simply
    // holds a lease while Wi-Fi is not routing.
    if !routed_wifi
        && (routed
            .as_deref()
            .and_then(|dev| ifaces.iter().find(|i| i.name == dev))
            .map(|i| i.kind == LinkKind::Ethernet)
            .unwrap_or(false)
            || ifaces
                .iter()
                .any(|i| i.kind == LinkKind::Ethernet && i.has_usable_ipv4()))
    {
        return (
            "network-wired-symbolic".into(),
            gettextrs::gettext("Wired"),
            None,
            None,
        );
    }

    (
        "network-offline-symbolic".into(),
        gettextrs::gettext("Offline"),
        None,
        None,
    )
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = i18n::init() {
        tracing::warn!("gettext initialization failed: {err:#}");
    }

    // XDG autostart launches the indicator at every login while the
    // systemd user bus outlives sessions — without a guard, stale
    // copies accumulate one tray icon per past login. The oldest
    // instance keeps running and keeps its single icon; any later
    // copy exits immediately.
    let _lock = match acquire_single_instance() {
        Some(conn) => conn,
        None => {
            tracing::info!("ragnar-heimdall-indicator already running — exiting");
            std::process::exit(0);
        }
    };

    let (icon, text, wifi, ssid) = probe();
    let tray = NetIndicator {
        icon,
        text,
        wifi,
        ssid,
    };
    let Ok(handle) = tray.spawn() else {
        // No SNI watcher reachable (e.g. running outside Ragnar) —
        // exiting cleanly beats spinning forever on a dead bus.
        std::process::exit(1);
    };

    loop {
        std::thread::sleep(Duration::from_secs(POLL_SECS));
        if handle.is_closed() {
            break;
        }
        let (icon, text, wifi, ssid) = probe();
        if handle
            .update(|t| {
                t.icon = icon;
                t.text = text;
                t.wifi = wifi;
                t.ssid = ssid;
            })
            .is_none()
        {
            break;
        }
    }
}
