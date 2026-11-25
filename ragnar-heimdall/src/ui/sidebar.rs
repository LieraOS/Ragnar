// Copyright (C) Jörg Schröder
//! Sidebar navigation: the [`View`] registry plus the `ListBox` that
//! renders one row per view with icon and label.

use gtk4::prelude::*;
use gtk4::{Box, Image, Label, ListBox, ListBoxRow, Orientation};

/// Every content view the application knows. Order defines the
/// sidebar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Interfaces,
    Routing,
    Dns,
    Vpn,
    Firewall,
    Diagnostics,
}

impl View {
    /// All views in sidebar order.
    pub const ALL: &'static [View] = &[
        View::Dashboard,
        View::Interfaces,
        View::Routing,
        View::Dns,
        View::Vpn,
        View::Firewall,
        View::Diagnostics,
    ];

    /// Stack page name for the view.
    pub fn tag(self) -> &'static str {
        match self {
            View::Dashboard => "dashboard",
            View::Interfaces => "interfaces",
            View::Routing => "routing",
            View::Dns => "dns",
            View::Vpn => "vpn",
            View::Firewall => "firewall",
            View::Diagnostics => "diagnostics",
        }
    }

    /// Freedesktop symbolic icon name for the row.
    pub fn icon(self) -> &'static str {
        match self {
            View::Dashboard => "view-grid-symbolic",
            View::Interfaces => "network-wired-symbolic",
            View::Routing => "network-transmit-receive-symbolic",
            View::Dns => "globe-symbolic",
            View::Vpn => "network-vpn-symbolic",
            View::Firewall => "security-high-symbolic",
            View::Diagnostics => "system-run-symbolic",
        }
    }

    /// Untranslated label; callers wrap it with `gettext`.
    pub fn label(self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Interfaces => "Interfaces",
            View::Routing => "Routing",
            View::Dns => "DNS",
            View::Vpn => "VPN",
            View::Firewall => "Firewall",
            View::Diagnostics => "Diagnostics",
        }
    }

    /// Resolve a row index back to its view.
    pub fn from_index(index: i32) -> Option<View> {
        usize::try_from(index)
            .ok()
            .and_then(|i| View::ALL.get(i))
            .copied()
    }
}

/// Build the styled sidebar list; `on_select` fires with the chosen
/// view whenever the user activates a row.
pub fn build<F: Fn(View) + 'static>(on_select: F) -> ListBox {
    let list = ListBox::builder().build();
    list.add_css_class("navigation-sidebar");

    for view in View::ALL {
        let row = sidebar_row(*view);
        list.append(&row);
    }

    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            if let Some(view) = View::from_index(row.index()) {
                on_select(view);
            }
        }
    });
    list
}

/// One sidebar row: icon + translated label.
fn sidebar_row(view: View) -> ListBoxRow {
    let icon = Image::from_icon_name(view.icon());
    icon.set_pixel_size(16);

    let label = Label::builder()
        .label(gettextrs::gettext(view.label()))
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .build();

    let content = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    content.append(&icon);
    content.append(&label);

    ListBoxRow::builder().child(&content).build()
}
