// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates page: checkbox list of pending updates, an install action
//! and a progress bar while the transaction runs.

use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::prelude::*;

use crate::packagekit::{severity_label, UpdateItem};

/// Row bookkeeping: checkbox widget → package id.
struct Row {
    check: gtk4::CheckButton,
    id: String,
}

/// The updates page widget plus its live parts.
pub struct UpdatesPage {
    pub widget: gtk4::Widget,
    list: gtk4::ListBox,
    install_btn: gtk4::Button,
    select_all: gtk4::CheckButton,
    progress: gtk4::ProgressBar,
    status: gtk4::Label,
    rows: Rc<RefCell<Vec<Row>>>,
    detail_title: gtk4::Label,
    detail_body: gtk4::Label,
    detail_pane: gtk4::Box,
    on_detail: Rc<dyn Fn(String)>,
}

impl UpdatesPage {
    /// Build the page. `on_install` fires with the checked package ids,
    /// `on_detail` fires with the clicked row's package id.
    pub fn new(
        on_install: impl Fn(Vec<String>) + 'static,
        on_detail: impl Fn(String) + 'static,
    ) -> Self {
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("boxed-list");
        let scroll = gtk4::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hexpand(true)
            .build();

        let select_all = gtk4::CheckButton::with_label(&gettext("Select all"));
        let install_btn = gtk4::Button::with_label(&gettext("Install selected"));
        install_btn.add_css_class("suggested-action");
        install_btn.set_sensitive(false);

        let progress = gtk4::ProgressBar::new();
        progress.set_visible(false);
        let status = gtk4::Label::new(None);
        status.set_halign(gtk4::Align::Start);
        status.add_css_class("dim-label");

        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        footer.append(&select_all);
        footer.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        footer.append(&install_btn);
        // Right-side detail pane: shows installed/new version and the
        // changelog text of the clicked update.
        let detail_title = gtk4::Label::new(None);
        detail_title.add_css_class("title-3");
        detail_title.set_halign(gtk4::Align::Start);
        detail_title.set_hexpand(true);
        let close_btn = gtk4::Button::from_icon_name("window-close-symbolic");
        close_btn.add_css_class("flat");
        let dhead = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        dhead.append(&detail_title);
        dhead.append(&close_btn);
        let detail_body = gtk4::Label::new(Some(&gettext("No update selected")));
        detail_body.set_halign(gtk4::Align::Start);
        detail_body.set_xalign(0.0);
        detail_body.set_wrap(true);
        detail_body.add_css_class("dim-label");
        let dscroll = gtk4::ScrolledWindow::builder()
            .child(&detail_body)
            .vexpand(true)
            .build();
        let detail_pane = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        detail_pane.set_margin_start(16);
        detail_pane.set_margin_top(12);
        detail_pane.append(&dhead);
        detail_pane.append(&dscroll);
        detail_pane.set_visible(false);
        detail_pane.set_width_request(180);
        detail_pane.set_hexpand(false);

        let hsplit = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        hsplit.set_vexpand(true);
        // Column header over the list so users know what the
        // columns mean: name+version on the left, severity right.
        let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let h_name = gtk4::Label::new(Some(&gettext("Package")));
        h_name.add_css_class("heading");
        h_name.set_hexpand(true);
        h_name.set_halign(gtk4::Align::Start);
        h_name.set_margin_start(48);
        let h_sev = gtk4::Label::new(Some(&gettext("Severity")));
        h_sev.add_css_class("heading");
        h_sev.set_margin_end(8);
        head.append(&h_name);
        head.append(&h_sev);
        let colbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        colbox.append(&head);
        colbox.append(&scroll);
        colbox.set_hexpand(true);
        hsplit.append(&colbox);
        hsplit.append(&detail_pane);

        let column = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        column.set_margin_start(18);
        column.set_margin_end(18);
        column.set_margin_top(12);
        column.append(&hsplit);
        column.append(&status);
        column.append(&footer);
        column.append(&progress);

        let on_detail: Rc<dyn Fn(String)> = Rc::new(on_detail);
        let page = Self {
            widget: column.upcast(),
            list,
            install_btn,
            select_all,
            progress,
            status,
            rows: Rc::new(RefCell::new(Vec::new())),
            detail_title,
            detail_body,
            detail_pane,
            on_detail,
        };
        // Close button folds the pane away again.
        let pane = page.detail_pane.clone();
        close_btn.connect_clicked(move |_| pane.set_visible(false));

        let rows = page.rows.clone();
        let btn = page.install_btn.clone();
        page.select_all.connect_toggled(move |sa| {
            let active = sa.is_active();
            for r in rows.borrow().iter() {
                r.check.set_active(active);
            }
            btn.set_sensitive(rows.borrow().iter().any(|r| r.check.is_active()));
        });

        let rows = page.rows.clone();
        page.install_btn.connect_clicked(move |b| {
            let ids: Vec<String> = rows
                .borrow()
                .iter()
                .filter(|r| r.check.is_active())
                .map(|r| r.id.clone())
                .collect();
            if !ids.is_empty() {
                b.set_sensitive(false);
                on_install(ids);
            }
        });

        page
    }

    /// Rebuild the list from a fresh update set.
    pub fn set_items(&self, items: &[UpdateItem]) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        self.rows.borrow_mut().clear();

        for it in items {
            let check = gtk4::CheckButton::new();
            check.set_active(false);
            let title = gtk4::Label::new(Some(&format!("{} {}", it.name, it.version)));
            title.set_halign(gtk4::Align::Start);
            title.set_xalign(0.0);
            let subtitle = gtk4::Label::new(Some(&it.summary));
            subtitle.set_halign(gtk4::Align::Start);
            subtitle.set_xalign(0.0);
            subtitle.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            subtitle.add_css_class("dim-label");
            let textcol = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            textcol.append(&title);
            textcol.append(&subtitle);

            // flatpak rows carry the source as their badge instead
            // of a PackageKit severity
            let badge = gtk4::Label::new(Some(&if it.flatpak {
                "Flathub".to_string()
            } else {
                severity_label(it.severity)
            }));
            badge.add_css_class("dim-label");
            if it.severity >= 7 {
                badge.add_css_class("warning");
            }
            if it.severity == 8 || it.severity == 9 {
                badge.add_css_class("error");
            }

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            hbox.append(&check);
            textcol.set_hexpand(true);
            hbox.append(&textcol);
            hbox.append(&badge);
            let row = gtk4::ListBoxRow::new();
            row.set_child(Some(&hbox));
            let info_btn = gtk4::Button::from_icon_name("dialog-information-symbolic");
            info_btn.add_css_class("flat");
            info_btn.set_valign(gtk4::Align::Center);
            let cb = self.on_detail.clone();
            let idc = it.id.clone();
            let pane = self.detail_pane.clone();
            info_btn.connect_clicked(move |_| {
                pane.set_visible(true);
                cb(idc.clone());
            });
            hbox.append(&info_btn);
            row.set_activatable(false);
            self.list.append(&row);

            let btn = self.install_btn.clone();
            let rows = self.rows.clone();
            let sa = self.select_all.clone();
            check.connect_toggled(move |c| {
                btn.set_sensitive(rows.borrow().iter().any(|r| r.check.is_active()));
                if !c.is_active() {
                    sa.set_active(false);
                }
            });
            self.rows.borrow_mut().push(Row {
                check,
                id: it.id.clone(),
            });
        }
        self.install_btn.set_sensitive(false);
        self.select_all.set_active(false);
        self.status.set_label(
            &gettextrs::ngettext("1 update", "%d updates", items.len() as u32)
                .replace("%d", &items.len().to_string()),
        );
    }

    /// Show the fetched detail of the clicked update.
    pub fn show_detail(&self, name: &str, installed: &str, d: &crate::packagekit::DetailInfo) {
        self.detail_pane.set_visible(true);
        // no "installed → updates" arrow when there is no version
        // pair to compare (flatpak remote-info path)
        self.detail_title.set_label(&if d.updates.is_empty() {
            name.to_string()
        } else {
            format!("{name} {} → {}", installed, d.updates)
        });
        let text = if d.text.is_empty() {
            gettext("No changelog available.")
        } else {
            d.text.clone()
        };
        self.detail_body.set_label(&text);
    }

    /// Detail fetch failed — tell the user in the pane.
    pub fn show_detail_error(&self, e: &str) {
        self.detail_pane.set_visible(true);
        self.detail_title.set_label(&gettext("Details"));
        self.detail_body
            .set_label(&gettext("Details could not be loaded: %s").replace("%s", e));
    }

    /// Show live install progress.
    pub fn set_progress(&self, pct: u32) {
        self.progress.set_visible(true);
        self.progress.set_fraction(pct as f64 / 100.0);
        self.status
            .set_label(&gettext("Installing … %d %%").replace("%d", &pct.to_string()));
    }

    /// Show which package is currently being installed.
    pub fn set_progress_name(&self, name: &str) {
        self.progress.set_visible(true);
        self.status
            .set_label(&gettext("Installing: %s").replace("%s", name));
    }

    /// Install finished — re-enable the button and show the outcome.
    pub fn install_done(&self, ok: bool, detail: &str) {
        self.progress.set_visible(false);
        self.install_btn.set_sensitive(true);
        let text = if ok {
            gettext("Installed: %s — PackageKit automatically installed required dependencies.")
                .replace("%s", detail)
        } else {
            gettext("Installation failed: %s").replace("%s", detail)
        };
        self.status.set_label(&text);
    }
}
