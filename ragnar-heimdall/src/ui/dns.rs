//! DNS page: shows systemd-resolved's global feature switches plus
//! every link's resolver state, lets the user set servers per link
//! (presets or custom), manage search domains, flush the cache and
//! run a live resolution test. Reads are unprivileged; mutations go
//! through pkexec + `ragnar-heimdall-helper`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, EntryRow, PreferencesGroup, PreferencesPage, Spinner, SwitchRow};
use gtk4::Image;

use crate::backend::{self, DnsLink, DnsStatus};

/// Poll interval for worker results arriving on the main loop.
const POLL_MS: u64 = 200;

/// Page state shared across async refresh/mutation callbacks.
struct Dns {
    page: PreferencesPage,
    groups: RefCell<Vec<PreferencesGroup>>,
    status: RefCell<Option<DnsStatus>>,
    busy: Cell<bool>,
    notice: RefCell<Option<String>>,
}

/// Build the DNS page. Without resolved the page is reduced to an
/// honest status group instead of dead controls.
pub fn build() -> gtk4::Widget {
    let page = PreferencesPage::new();
    let dns = Rc::new(Dns {
        page: page.clone(),
        groups: RefCell::new(Vec::new()),
        status: RefCell::new(None),
        busy: Cell::new(false),
        notice: RefCell::new(None),
    });

    if !backend::dns_available() {
        let group = PreferencesGroup::builder()
            .title(gettextrs::gettext("DNS"))
            .build();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("systemd-resolved is not installed"))
            .subtitle(gettextrs::gettext(
                "DNS configuration needs a running systemd-resolved.",
            ))
            .build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        page.add(&group);
        return page.upcast();
    }

    refresh(Rc::clone(&dns));
    page.upcast()
}

/// (Re)load `resolvectl -j status` in a worker, then re-render.
fn refresh(dns: Rc<Dns>) {
    dns.busy.set(true);
    render(&dns);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(backend::dns_status());
    });
    let dns2 = Rc::clone(&dns);
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                dns2.busy.set(false);
                match result {
                    Ok(status) => {
                        *dns2.status.borrow_mut() = Some(status);
                        *dns2.notice.borrow_mut() = None;
                    }
                    Err(err) => {
                        tracing::warn!("resolvectl status failed: {err:#}");
                        *dns2.notice.borrow_mut() = Some(format!("{err:#}"));
                    }
                }
                render(&dns2);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Run a mutating resolvectl call in a worker, then refresh.
fn mutate<F: FnOnce() -> anyhow::Result<()> + Send + 'static>(dns: Rc<Dns>, op: F) {
    dns.busy.set(true);
    render(&dns);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = op();
        let _ = tx.send(result);
    });
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(result) => {
                if let Err(err) = result {
                    tracing::warn!("resolvectl failed: {err:#}");
                    *dns.notice.borrow_mut() = Some(format!("{err:#}"));
                    dns.busy.set(false);
                    render(&dns);
                } else {
                    refresh(dns.clone());
                }
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Rebuild all groups from the current state.
fn render(dns: &Rc<Dns>) {
    for group in dns.groups.borrow_mut().drain(..) {
        dns.page.remove(&group);
    }

    if let Some(msg) = dns.notice.borrow().clone() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder().subtitle(msg).build();
        row.add_prefix(&Image::from_icon_name("dialog-warning-symbolic"));
        group.add(&row);
        add_group(dns, &group);
    }

    if dns.busy.get() {
        let group = PreferencesGroup::new();
        let row = ActionRow::builder()
            .title(gettextrs::gettext("Working…"))
            .build();
        row.add_suffix(&Spinner::new());
        group.add(&row);
        add_group(dns, &group);
        return;
    }

    let Some(status) = dns.status.borrow().clone() else {
        let group = PreferencesGroup::new();
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("DNS state could not be read"))
                .build(),
        );
        add_group(dns, &group);
        return;
    };

    add_group(dns, &global_group(dns, &status));
    for link in &status.links {
        add_group(dns, &link_group(dns, link));
    }
    add_group(dns, &tools_group(dns));
}

/// Track a freshly added group for the next render pass.
fn add_group(dns: &Rc<Dns>, group: &PreferencesGroup) {
    dns.page.add(group);
    dns.groups.borrow_mut().push(group.clone());
}

/// Group 1: the four global resolved feature switches.
fn global_group(dns: &Rc<Dns>, status: &DnsStatus) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Global DNS settings"))
        .description(format!("resolv.conf: {}", status.global.resolv_conf_mode))
        .build();

    group.add(&feature_switch(
        dns,
        gettextrs::gettext("DNSSEC validation"),
        gettextrs::gettext("Verify DNS answers cryptographically."),
        status.global.dnssec == "yes",
        status.global.dnssec_supported,
        "dnssec",
    ));
    group.add(&feature_switch(
        dns,
        gettextrs::gettext("DNS over TLS"),
        gettextrs::gettext("Encrypt DNS traffic (needs a resolver that supports it)."),
        status.global.dns_over_tls == "yes",
        true,
        "dnsovertls",
    ));
    group.add(&feature_switch(
        dns,
        gettextrs::gettext("LLMNR"),
        gettextrs::gettext("Resolve local hostnames on the LAN."),
        status.global.llmnr == "yes",
        true,
        "llmnr",
    ));
    group.add(&feature_switch(
        dns,
        gettextrs::gettext("mDNS"),
        gettextrs::gettext("Resolve .local names (Bonjour/Avahi style)."),
        status.global.mdns == "yes",
        true,
        "mdns",
    ));
    group
}

/// One global feature switch; unsupported features render disabled.
fn feature_switch(
    dns: &Rc<Dns>,
    title: String,
    subtitle: String,
    active: bool,
    supported: bool,
    verb: &'static str,
) -> SwitchRow {
    let row = SwitchRow::builder()
        .title(title)
        .subtitle(if supported {
            subtitle
        } else {
            gettextrs::gettext("Not supported by this resolved build.")
        })
        .active(active && supported)
        .sensitive(supported)
        .build();
    let dns = Rc::clone(dns);
    row.connect_active_notify(move |sw| {
        let value = if sw.is_active() { "yes" } else { "no" };
        mutate(Rc::clone(&dns), move || {
            backend::dns_set_feature(verb, value)
        });
    });
    row
}

/// One group per link: answering server, full list, domains, actions.
fn link_group(dns: &Rc<Dns>, link: &DnsLink) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(format!(
            "{} ({})",
            gettextrs::gettext("Resolvers"),
            link.ifname
        ))
        .build();

    // Which server actually answers — resolved picks one at a time.
    let current = link
        .current_server
        .clone()
        .unwrap_or_else(|| gettextrs::gettext("(none answering)"));
    group.add(
        &ActionRow::builder()
            .title(gettextrs::gettext("Current DNS server"))
            .subtitle(current)
            .build(),
    );

    for srv in &link.servers {
        let row = ActionRow::builder().title(srv).build();
        row.add_prefix(&Image::from_icon_name("globe-symbolic"));
        group.add(&row);
    }
    if link.servers.is_empty() {
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("No DNS servers configured"))
                .build(),
        );
    }

    if !link.domains.is_empty() {
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("Search domains"))
                .subtitle(link.domains.join(" "))
                .build(),
        );
    }

    if link.default_route {
        group.add(
            &ActionRow::builder()
                .title(gettextrs::gettext("Used for all lookups"))
                .subtitle(gettextrs::gettext("Default route"))
                .build(),
        );
    }

    let set = ActionRow::builder()
        .title(gettextrs::gettext("Set DNS servers…"))
        .activatable(true)
        .build();
    set.add_prefix(&Image::from_icon_name("document-edit-symbolic"));
    {
        let dns = Rc::clone(dns);
        let ifname = link.ifname.clone();
        set.connect_activated(move |row| {
            let dns2 = Rc::clone(&dns);
            let ifn = ifname.clone();
            super::dns_add::present(row, &ifname, move |servers| {
                let ifn2 = ifn.clone();
                mutate(Rc::clone(&dns2), move || {
                    backend::dns_set_servers(&ifn2, &servers)
                });
            });
        });
    }
    group.add(&set);

    let revert = ActionRow::builder()
        .title(gettextrs::gettext("Back to automatic (DHCP)"))
        .activatable(true)
        .build();
    revert.add_prefix(&Image::from_icon_name("edit-undo-symbolic"));
    {
        let dns = Rc::clone(dns);
        let ifname = link.ifname.clone();
        revert.connect_activated(move |_| {
            let ifn = ifname.clone();
            mutate(Rc::clone(&dns), move || backend::dns_set_servers(&ifn, &[]));
        });
    }
    group.add(&revert);
    group
}

/// Group N: cache flush plus the live resolution test.
fn tools_group(dns: &Rc<Dns>) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Tools"))
        .build();

    let flush = ActionRow::builder()
        .title(gettextrs::gettext("Flush DNS cache"))
        .subtitle(gettextrs::gettext(
            "Drops all cached answers — helps after stale or poisoned entries.",
        ))
        .activatable(true)
        .build();
    flush.add_prefix(&Image::from_icon_name("view-refresh-symbolic"));
    {
        let dns = Rc::clone(dns);
        flush.connect_activated(move |_| {
            mutate(Rc::clone(&dns), backend::dns_flush_caches);
        });
    }
    group.add(&flush);

    // Live lookup: type a name, get addresses + resolver latency.
    let entry = EntryRow::builder()
        .title(gettextrs::gettext("Test resolution"))
        .build();
    let result = ActionRow::builder()
        .title(gettextrs::gettext("Result"))
        .subtitle(gettextrs::gettext("Type a name and press Enter"))
        .build();
    group.add(&entry);
    group.add(&result);
    {
        let result = result.clone();
        entry.connect_entry_activated(move |e| {
            let name = e.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            result.set_subtitle(&gettextrs::gettext("Querying…"));
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(backend::dns_query(&name));
            });
            let result = result.clone();
            gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
                match rx.try_recv() {
                    Ok(Ok((addrs, ms))) => {
                        result.set_subtitle(&format!(
                            "{} · {ms} ms",
                            if addrs.is_empty() {
                                gettextrs::gettext("(no addresses returned)")
                            } else {
                                addrs.join(", ")
                            }
                        ));
                        gtk4::glib::ControlFlow::Break
                    }
                    Ok(Err(err)) => {
                        result.set_subtitle(&format!("{err:#}"));
                        gtk4::glib::ControlFlow::Break
                    }
                    Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
                    Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
                }
            });
        });
    }
    group
}
