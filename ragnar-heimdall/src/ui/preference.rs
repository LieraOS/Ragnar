//! Global connection preference — one combo steering the route
//! metrics of ALL physical links at once. Under the hood it writes
//! the same RouteMetric drop-ins the per-interface combo uses, so
//! both levels stay consistent: the preset is the coarse dial, the
//! per-link combo the fine one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use libadwaita as adw;

use adw::prelude::*;
use adw::PreferencesGroup;

use crate::backend::{self, LinkKind, NetworkInterface};

/// Metric presets mirrored by the per-interface combo.
const PREFERRED: u32 = 100;
const FALLBACK: u32 = 600;

/// Coarse user-facing preference derived from live metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preference {
    Auto,
    Lan,
    Wifi,
    Custom,
}

/// Deduce the current preference from the link snapshot: preset
/// states only when ALL physical links carry the matching metrics —
/// anything mixed counts as a custom per-interface setup.
fn current(ifaces: &[NetworkInterface]) -> Preference {
    let wired: Vec<_> = ifaces
        .iter()
        .filter(|i| i.kind == LinkKind::Ethernet)
        .collect();
    let wifi: Vec<_> = ifaces.iter().filter(|i| i.kind == LinkKind::Wifi).collect();

    if wired.iter().all(|i| i.route_metric.is_none())
        && wifi.iter().all(|i| i.route_metric.is_none())
    {
        return Preference::Auto;
    }
    if !wired.is_empty()
        && wired.iter().all(|i| i.route_metric == Some(PREFERRED))
        && wifi.iter().all(|i| i.route_metric == Some(FALLBACK))
    {
        return Preference::Lan;
    }
    if !wifi.is_empty()
        && wifi.iter().all(|i| i.route_metric == Some(PREFERRED))
        && wired.iter().all(|i| i.route_metric == Some(FALLBACK))
    {
        return Preference::Wifi;
    }
    Preference::Custom
}

/// Build the preference group for the interfaces page. `latest`
/// holds the most recent snapshot (shared with the refresh loop)
/// so apply/refresh both see live state.
pub fn group(latest: &Rc<RefCell<Vec<NetworkInterface>>>) -> PreferencesGroup {
    let labels = [
        gettextrs::gettext("Automatic (fastest link first)"),
        gettextrs::gettext("Prefer wired (LAN)"),
        gettextrs::gettext("Prefer Wi-Fi"),
    ];
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    let combo = adw::ComboRow::builder()
        .title(gettextrs::gettext("Connection preference"))
        .model(&gtk4::StringList::new(&refs))
        .build();

    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Connection preference"))
        .description(gettextrs::gettext(
            "Which link should be used first when several are up. \
             Wired is usually faster and more stable.",
        ))
        .build();
    group.add(&combo);

    // `suppress` guards programmatic `set_selected` — GTK fires
    // selected_notify for code-driven changes exactly like clicks,
    // so without it every resync would re-trigger an apply and the
    // combo would bounce back to the old preset while writing.
    let suppress = Rc::new(Cell::new(false));
    // `in_flight` pauses snapshot resync while a preset is being
    // written — the combo keeps the user's pick until metrics land.
    let in_flight = Arc::new(AtomicBool::new(false));

    // Reflect the snapshot state — including the "Custom" hint when
    // per-interface metrics don't match any preset.
    let apply_state = {
        let combo = combo.clone();
        let suppress = Rc::clone(&suppress);
        move |ifaces: &[NetworkInterface]| {
            suppress.set(true);
            match current(ifaces) {
                Preference::Auto => combo.set_selected(0),
                Preference::Lan => combo.set_selected(1),
                Preference::Wifi => combo.set_selected(2),
                Preference::Custom => combo.set_subtitle(&gettextrs::gettext(
                    "Custom per-interface priorities active — choosing a preset overrides them.",
                )),
            }
            if current(ifaces) != Preference::Custom {
                combo.set_subtitle("");
            }
            suppress.set(false);
        }
    };
    {
        let ifaces = latest.borrow();
        apply_state(&ifaces);
    }
    {
        let latest = Rc::clone(latest);
        let in_flight = Arc::clone(&in_flight);
        // Re-sync the combo whenever the refresh loop drops a new
        // snapshot — external metric edits stay visible. Skipped
        // while an apply is in flight so the user's pick stays put.
        gtk4::glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
            if !in_flight.load(Ordering::Relaxed) {
                apply_state(&latest.borrow());
            }
            gtk4::glib::ControlFlow::Continue
        });
    }

    {
        let latest = Rc::clone(latest);
        let suppress = Rc::clone(&suppress);
        combo.connect_selected_notify(move |c| {
            if suppress.get() {
                return;
            }
            let ifaces = latest.borrow().clone();
            let selection = c.selected();
            in_flight.store(true, Ordering::Relaxed);
            let in_flight2 = Arc::clone(&in_flight);
            // One worker applies the whole preset — pkexec keeps the
            // auth for the burst so it prompts at most once.
            std::thread::spawn(move || {
                for i in &ifaces {
                    let metric = match (selection, i.kind) {
                        (0, LinkKind::Ethernet | LinkKind::Wifi) => None,
                        (1, LinkKind::Ethernet) => Some(PREFERRED),
                        (1, LinkKind::Wifi) => Some(FALLBACK),
                        (2, LinkKind::Wifi) => Some(PREFERRED),
                        (2, LinkKind::Ethernet) => Some(FALLBACK),
                        _ => continue,
                    };
                    if i.route_metric == metric {
                        continue;
                    }
                    if let Err(err) = backend::set_route_metric(&i.name, metric) {
                        tracing::warn!("route metric on {} failed: {err:#}", i.name);
                    }
                }
                in_flight2.store(false, Ordering::Relaxed);
            });
        });
    }
    group
}
