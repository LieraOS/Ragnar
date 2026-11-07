//! Routing page: shows the kernel routing tables (IPv4 + IPv6) and
//! the policy rules, and lets the user add or delete routes. Reads
//! are unprivileged `ip -j` snapshots; mutations go through pkexec +
//! `ragnar-heimdall-helper` exactly like the firewall page.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, AlertDialog, PreferencesGroup, PreferencesPage, Spinner};
use gtk4::{Align, Button, Image};

use crate::backend::{self, Route, Rule};

/// Poll interval for worker results arriving on the main loop.
const POLL_MS: u64 = 200;

/// Page state shared across async refresh/mutation callbacks.
struct Rt {
    page: PreferencesPage,
    groups: RefCell<Vec<PreferencesGroup>>,
    /// `(v4, v6)` routes plus policy rules from the last snapshot.
    data: RefCell<Option<(Vec<Route>, Vec<Rule>)>>,
    busy: Cell<bool>,
    notice: RefCell<Option<String>>,
}

/// Build the routing page from a fresh `ip -j` snapshot.
pub fn build() -> gtk4::Widget {
    let page = PreferencesPage::new();
    let rt = Rc::new(Rt {
        page: page.clone(),
        groups: RefCell::new(Vec::new()),
        data: RefCell::new(None),
        busy: Cell::new(false),
        notice: RefCell::new(None),
    });

    // Static header group: explains the page and offers the only
    // write action — adding a route. Everything below is data.
    let actions = PreferencesGroup::builder()
        .title(gettextrs::gettext("Routing"))
        .description(gettextrs::gettext(
            "Where outgoing packets are sent. Kernel-managed entries \
             update on their own; manual routes survive until removed.",
        ))
        .build();
    let add = ActionRow::builder()
        .title(gettextrs::gettext("Add route…"))
        .activatable(true)
        .build();
    add.add_prefix(&Image::from_icon_name("list-add-symbolic"));
    {
        let rt2 = Rc::clone(&rt);
        add.connect_activated(move |_| {
            let rt3 = Rc::clone(&rt2);
            let rt4 = Rc::clone(&rt3);
            super::routing_add::present(&rt3.page, move |dst, gw, dev, metric| {
                mutate(Rc::clone(&rt4), move || {
                    backend::route_add(&dst, &gw, &dev, &metric)
                });
            });
        });
    }
    actions.add(&add);
    page.add(&actions);
    rt.groups.borrow_mut().push(actions);

    // First snapshot: `ip -j` is fast enough to read synchronously,
    // but still runs in a worker so a hung `ip` can never freeze GTK.
    refresh(Rc::clone(&rt));
    page.upcast()
}

/// (Re)load routes + rules in a worker thread, then re-render.
fn refresh(rt: Rc<Rt>) {
    rt.busy.set(true);
    render(&rt);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(backend::routes().and_then(|r| backend::rules().map(|rules| (r, rules))));
    });
    let rt2 = Rc::clone(&rt);
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                rt2.busy.set(false);
                match result {
                    Ok(data) => {
                        *rt2.data.borrow_mut() = Some(data);
                        *rt2.notice.borrow_mut() = None;
                    }
                    Err(err) => {
                        tracing::warn!("routing snapshot failed: {err:#}");
                        *rt2.notice.borrow_mut() = Some(format!("{err:#}"));
                    }
                }
                render(&rt2);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Run a mutating `ip route` call in a worker, then refresh.
fn mutate<F: FnOnce() -> anyhow::Result<()> + Send + 'static>(rt: Rc<Rt>, op: F) {
    rt.busy.set(true);
    render(&rt);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = op();
        let _ = tx.send(result);
    });
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                if let Err(err) = result {
                    tracing::warn!("ip route failed: {err:#}");
                    *rt.notice.borrow_mut() = Some(format!("{err:#}"));
                    rt.busy.set(false);
                    render(&rt);
                    gtk4::glib::ControlFlow::Break
                } else {
                    refresh(rt.clone());
                    gtk4::glib::ControlFlow::Break
                }
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Rebuild all data groups from the current state. The first group
/// (index 0) is the static actions group and survives every pass.
fn render(rt: &Rc<Rt>) {
    // Drop previously rendered data groups, keep the actions group.
    for group in rt.groups.borrow_mut().drain(1..) {
        rt.page.remove(&group);
    }

    if let Some(msg) = rt.notice.borrow().clone() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder().subtitle(msg).build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        add_group(rt, &group);
    }

    if rt.busy.get() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("Working…"))
            .build();
        row.add_suffix(&Spinner::new());
        group.add(&row);
        add_group(rt, &group);
        return;
    }

    let Some((routes, rules)) = rt.data.borrow().clone() else {
        return;
    };

    // Group routes per table and family: `main` first, then every
    // other table alphabetically. The kernel-owned `local` table is
    // shown read-only — its broadcast entries answer "why is there
    // a 255.255.255.255 route" without inviting deletion.
    let mut tables: BTreeMap<String, Vec<Route>> = BTreeMap::new();
    for r in &routes {
        tables.entry(r.table.clone()).or_default().push(r.clone());
    }
    let mut names: Vec<&String> = tables.keys().collect();
    names.sort_by_key(|n| {
        if *n == "main" {
            String::new()
        } else {
            (*n).clone()
        }
    });

    for name in names {
        let Some(list) = tables.get(name) else {
            continue;
        };
        let managed = name == "local";
        let group = PreferencesGroup::builder()
            .title(if *name == "main" {
                gettextrs::gettext("Routing table (main)")
            } else {
                format!("{} ({name})", gettextrs::gettext("Routing table"))
            })
            .description(if managed {
                gettextrs::gettext("Kernel-managed broadcast routes — shown for completeness.")
            } else {
                String::new()
            })
            .build();
        let mut any = false;
        // IPv4 entries first — the common case reads top-down.
        for route in list
            .iter()
            .filter(|r| !r.ipv6)
            .chain(list.iter().filter(|r| r.ipv6))
        {
            // Kernel-installed connected routes are auto-maintained;
            // deleting them only breaks the address assignment, so
            // they render read-only like the local table.
            let ro =
                managed || (route.protocol == "kernel" && route.scope.as_deref() == Some("link"));
            group.add(&route_row(rt, route, ro));
            any = true;
        }
        if any {
            add_group(rt, &group);
        }
    }

    // Policy rules decide which table a packet consults — display
    // them in lookup order so the precedence chain reads top-down.
    let rg = PreferencesGroup::builder()
        .title(gettextrs::gettext("Policy rules"))
        .description(gettextrs::gettext(
            "Checked top to bottom; the first match selects the routing table.",
        ))
        .build();
    for rule in rules {
        rg.add(&rule_row(&rule));
    }
    add_group(rt, &rg);
}

/// Track a freshly added group for the next render pass.
fn add_group(rt: &Rc<Rt>, group: &PreferencesGroup) {
    rt.page.add(group);
    rt.groups.borrow_mut().push(group.clone());
}

/// One route row: destination, next hop/interface details, delete
/// button unless the row is kernel-managed.
fn route_row(rt: &Rc<Rt>, route: &Route, managed: bool) -> ActionRow {
    let mut subtitle = String::new();
    if let Some(gw) = &route.gateway {
        subtitle.push_str(&format!("via {gw} · "));
    }
    if let Some(dev) = &route.dev {
        subtitle.push_str(&format!("dev {dev} · "));
    }
    subtitle.push_str(&route.protocol);
    if let Some(scope) = &route.scope {
        subtitle.push_str(&format!(" · scope {scope}"));
    }
    if let Some(m) = route.metric {
        subtitle.push_str(&format!(" · metric {m}"));
    }
    if let Some(src) = &route.prefsrc {
        subtitle.push_str(&format!(" · src {src}"));
    }

    let mut title = route.dst.clone();
    if route.ipv6 {
        title.push_str(" (IPv6)");
    }
    let row = ActionRow::builder()
        .title(&title)
        .subtitle(&subtitle)
        .build();
    row.add_prefix(&Image::from_icon_name(if route.dst == "default" {
        "network-transmit-receive-symbolic"
    } else {
        "go-next-symbolic"
    }));

    if !managed {
        let trash = Button::builder()
            .icon_name("user-trash-symbolic")
            .valign(Align::Center)
            .tooltip_text(gettextrs::gettext("Delete route"))
            .build();
        trash.add_css_class("flat");
        {
            let rt = Rc::clone(rt);
            let route = route.clone();
            trash.connect_clicked(move |btn| {
                confirm_delete(&rt, &route, btn);
            });
        }
        row.add_suffix(&trash);
    }
    row
}

/// One policy rule: lookup priority, selector and target table.
fn rule_row(rule: &Rule) -> ActionRow {
    let row = ActionRow::builder()
        .title(format!("{} · {}", rule.priority, rule.selector))
        .subtitle(format!(
            "{} {}",
            gettextrs::gettext("lookup table"),
            rule.table
        ))
        .build();
    row.add_prefix(&Image::from_icon_name("view-list-ordered-symbolic"));
    row
}

/// Deleting the default route or a whole subnet can cut the machine
/// off — confirm before the privileged call happens.
fn confirm_delete(rt: &Rc<Rt>, route: &Route, source: &Button) {
    let dialog = AlertDialog::builder()
        .heading(gettextrs::gettext("Delete route?"))
        .body(if route.dst == "default" {
            gettextrs::gettext(
                "This is the default gateway route. Deleting it disconnects \
                 the machine from the internet until a new route appears.",
            )
        } else {
            format!(
                "{} {}",
                gettextrs::gettext("This removes the route to"),
                route.dst
            )
        })
        .build();
    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("delete", &gettextrs::gettext("Delete"));
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let rt = Rc::clone(rt);
    let route = route.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "delete" {
            return;
        }
        let r = route.clone();
        mutate(Rc::clone(&rt), move || backend::route_delete(&r));
    });
    dialog.present(Some(source));
}
