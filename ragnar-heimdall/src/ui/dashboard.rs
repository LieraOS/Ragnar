//! Dashboard view: live status summary built from real system facts.

use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, PreferencesGroup, PreferencesPage};

use crate::backend;

/// Build the dashboard content, reading the current system state.
/// Failures degrade to a visible status row instead of panicking.
pub fn build() -> gtk4::Widget {
    let page = PreferencesPage::new();
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Network Status"))
        .build();
    page.add(&group);

    group.add(&row(
        "network-workgroup-symbolic",
        &gettextrs::gettext("Hostname"),
        &backend::hostname(),
    ));

    match backend::default_route() {
        Ok(Some(route)) if !route.is_empty() => group.add(&row(
            "network-transmit-receive-symbolic",
            &gettextrs::gettext("Connectivity"),
            &format!("{} ({route})", gettextrs::gettext("Online")),
        )),
        _ => group.add(&row(
            "network-offline-symbolic",
            &gettextrs::gettext("Connectivity"),
            &gettextrs::gettext("No default route"),
        )),
    }

    let dns = backend::dns_servers();
    let dns_text = if dns.is_empty() {
        gettextrs::gettext("None configured")
    } else {
        dns.join(", ")
    };
    group.add(&row(
        "globe-symbolic",
        &gettextrs::gettext("DNS Servers"),
        &dns_text,
    ));

    match backend::collect_interfaces() {
        Ok(interfaces) => {
            let up = interfaces
                .iter()
                .filter(|i| {
                    i.state == backend::LinkState::Up && i.kind != backend::LinkKind::Loopback
                })
                .count();
            let total = interfaces
                .iter()
                .filter(|i| i.kind != backend::LinkKind::Loopback)
                .count();
            group.add(&row(
                "network-wired-symbolic",
                &gettextrs::gettext("Interfaces Up"),
                &format!("{up} / {total}"),
            ));
        }
        Err(err) => {
            tracing::warn!("interface collection failed: {err:#}");
            group.add(&row(
                "dialog-warning-symbolic",
                &gettextrs::gettext("Interfaces"),
                &gettextrs::gettext("Could not read interface state"),
            ));
        }
    }

    page.upcast()
}

/// One labelled status row with an icon.
fn row(icon: &str, title: &str, value: &str) -> ActionRow {
    let row = ActionRow::builder().title(title).subtitle(value).build();
    row.add_prefix(&gtk4::Image::from_icon_name(icon));
    row
}
