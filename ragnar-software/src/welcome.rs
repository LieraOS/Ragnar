// SPDX-License-Identifier: GPL-3.0-or-later
//! Welcome page: the Windows-Update-style landing view. Shows the
//! current update status (checked in the background at startup) and
//! offers a manual "check now" action.

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;

/// The welcome page widget plus its updatable bits.
pub struct WelcomePage {
    pub widget: adw::StatusPage,
    status: gtk4::Label,
    spinner: gtk4::Spinner,
}

impl WelcomePage {
    /// Build the page.
    pub fn new() -> Self {
        let status = gtk4::Label::new(Some(&gettext("Checking update status …")));
        status.add_css_class("dim-label");

        let spinner = gtk4::Spinner::new();
        spinner.set_spinning(true);
        spinner.set_halign(gtk4::Align::Center);

        let page = adw::StatusPage::builder()
            .icon_name("system-software-install-symbolic")
            .title(gettext("Ragnar Software"))
            .description(gettext("System updates and software management"))
            .build();
        page.set_vexpand(true);
        page.set_valign(gtk4::Align::Fill);

        let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        body.append(&spinner);
        body.append(&status);
        page.set_child(Some(&body));

        Self {
            widget: page,
            status,
            spinner,
        }
    }

    /// Show the pending-update count (or "system is up to date").
    pub fn set_count(&self, n: usize) {
        self.spinner.set_visible(false);
        let text = if n == 0 {
            gettext("The system is up to date.")
        } else {
            gettextrs::ngettext("1 update available", "%d updates available", n as u32)
                .replace("%d", &n.to_string())
        };
        self.status.set_label(&text);
        self.status.remove_css_class("dim-label");
        if n > 0 {
            self.status.add_css_class("accent");
        }
    }

    /// Show a fetch failure.
    pub fn set_error(&self, e: &str) {
        self.spinner.set_visible(false);
        self.status
            .set_label(&gettext("Failed to check status: %s").replace("%s", e));
        self.status.add_css_class("error");
    }

    /// Back to the checking state (manual recheck).
    pub fn set_checking(&self) {
        self.spinner.set_visible(true);
        self.status.set_label(&gettext("Checking update status …"));
    }
}
