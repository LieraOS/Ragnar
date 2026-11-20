// SPDX-License-Identifier: GPL-3.0-or-later
//! Software catalogue page: main categories on the left, app list on
//! the right, and a search entry that filters the loaded AppStream
//! components. Only real applications (desktop-application
//! components) appear — libraries never reach this view.

use std::cell::RefCell;
use std::rc::Rc;

use crate::i18n::msgid;
use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::catalog_data::AppItem;

/// Top-level groups shown in the sidebar; each maps AppStream
/// category names to a display label.
const GROUPS: &[(&str, &[&str])] = &[
    (
        msgid("Audio & Video"),
        &[
            "AudioVideo",
            "Audio",
            "Video",
            "Player",
            "Recorder",
            "Multimedia",
        ],
    ),
    (
        msgid("Image editing"),
        &[
            "Graphics",
            "Photography",
            "RasterGraphics",
            "VectorGraphics",
        ],
    ),
    (msgid("Development"), &["Development", "IDE", "Debugger"]),
    (msgid("Education"), &["Education"]),
    (msgid("Games"), &["Game"]),
    (
        msgid("Internet"),
        &["Network", "WebBrowser", "Email", "InstantMessaging"],
    ),
    (msgid("Office"), &["Office"]),
    (msgid("Science"), &["Science"]),
    (msgid("System"), &["System", "Settings"]),
    (msgid("Utilities"), &["Utility"]),
];

pub struct CatalogPage {
    pub widget: adw::NavigationPage,
    apps: Rc<RefCell<Vec<AppItem>>>,
    list: gtk4::ListBox,
    group_list: gtk4::ListBox,
    title: gtk4::Label,
    search: gtk4::SearchEntry,
    on_install: Rc<dyn Fn((String, String))>,
    on_remove: Rc<dyn Fn((String, String))>,
    detail_title: gtk4::Label,
    detail_body: gtk4::Label,
    detail_icon: gtk4::Image,
    detail_pane: gtk4::Box,
    installed: Rc<RefCell<std::collections::HashSet<String>>>,
    status: gtk4::Label,
    scroll: gtk4::ScrolledWindow,
    filter: Rc<RefCell<u8>>,
}

impl CatalogPage {
    /// Build the page. `on_install` fires with a package name.
    pub fn new(
        on_install: impl Fn((String, String)) + 'static,
        on_remove: impl Fn((String, String)) + 'static,
    ) -> Self {
        let on_install: Rc<dyn Fn((String, String))> = Rc::new(on_install);
        let on_remove: Rc<dyn Fn((String, String))> = Rc::new(on_remove);
        let group_list = gtk4::ListBox::new();
        group_list.add_css_class("navigation-sidebar");
        for (label, _) in GROUPS {
            let row = gtk4::ListBoxRow::new();
            let lbl = gtk4::Label::new(Some(&gettext(*label)));
            lbl.set_halign(gtk4::Align::Start);
            {
                lbl.set_margin_start(8);
                lbl.set_margin_end(8);
                lbl.set_margin_top(8);
                lbl.set_margin_bottom(8);
            }
            row.set_child(Some(&lbl));
            row.set_widget_name(label);
            group_list.append(&row);
        }

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        let scroll = gtk4::ScrolledWindow::builder()
            .child(&list)
            .hexpand(true)
            .vexpand(true)
            .build();

        // right-side detail pane — opens on the info button
        let detail_title = gtk4::Label::new(None);
        detail_title.add_css_class("title-3");
        detail_title.set_halign(gtk4::Align::Start);
        detail_title.set_hexpand(true);
        let detail_close = gtk4::Button::from_icon_name("window-close-symbolic");
        detail_close.add_css_class("flat");
        let dhead = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let detail_icon = gtk4::Image::new();
        detail_icon.set_pixel_size(48);
        detail_icon.set_valign(gtk4::Align::Start);
        dhead.append(&detail_icon);
        dhead.append(&detail_title);
        dhead.append(&detail_close);
        let detail_body = gtk4::Label::new(Some(&gettext("No app selected")));
        detail_body.set_halign(gtk4::Align::Start);
        detail_body.set_xalign(0.0);
        detail_body.set_wrap(true);
        detail_body.set_selectable(true);
        detail_body.add_css_class("dim-label");
        let dscroll = gtk4::ScrolledWindow::builder()
            .child(&detail_body)
            .vexpand(true)
            .build();
        let detail_pane = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        detail_pane.set_margin_start(16);
        detail_pane.set_margin_top(12);
        detail_pane.set_margin_end(8);
        detail_pane.append(&dhead);
        detail_pane.append(&dscroll);
        detail_pane.set_visible(false);
        detail_pane.set_width_request(240);
        detail_pane.set_hexpand(false);

        let title = gtk4::Label::new(Some(&gettext("Categories")));
        title.add_css_class("title-3");
        title.set_halign(gtk4::Align::Start);
        let src_filter = gtk4::DropDown::from_strings(&[
            &gettext("All"),
            &gettext("Debian"),
            &gettext("Flathub"),
        ]);
        let search = gtk4::SearchEntry::new();
        search.set_hexpand(true);
        search.set_placeholder_text(Some(&gettext("Search packages")));

        let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        head.set_margin_start(8);
        head.set_margin_end(8);
        head.set_margin_top(8);
        head.set_margin_bottom(8);
        head.append(&title);
        head.append(&src_filter);
        head.append(&search);

        let status_lbl = gtk4::Label::new(None);
        let status = status_lbl.clone();
        status.set_halign(gtk4::Align::Start);
        status.add_css_class("dim-label");
        status.set_margin_start(8);
        status.set_margin_bottom(6);
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        content.append(&head);
        // split row: app list left, collapsible detail pane right
        let hsplit = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        hsplit.set_vexpand(true);
        hsplit.append(&scroll);
        hsplit.append(&detail_pane);
        content.append(&hsplit);
        content.append(&status);

        let split = adw::NavigationSplitView::builder()
            .sidebar(&adw::NavigationPage::new(&group_list, &gettext("Software")))
            .content(&adw::NavigationPage::new(&content, &gettext("Software")))
            .min_sidebar_width(200.0)
            .build();
        let page = adw::NavigationPage::new(&split, &gettext("Software"));

        let apps: Rc<RefCell<Vec<AppItem>>> = Rc::new(RefCell::new(Vec::new()));
        let this = Self {
            widget: page,
            apps,
            list,
            group_list,
            title,
            search,
            on_install: on_install.clone(),
            on_remove: on_remove.clone(),
            detail_title,
            detail_body,
            detail_icon,
            detail_pane,
            installed: Rc::new(RefCell::new(std::collections::HashSet::new())),
            status,
            scroll,
            filter: Rc::new(RefCell::new(0)),
        };
        // Sidebar selection → show that group's apps.
        let apps2 = this.apps.clone();
        let list2 = this.list.clone();
        let title2 = this.title.clone();
        let oi2 = on_install.clone();
        let inst2 = this.installed.clone();
        let orm2 = on_remove.clone();
        let flt2 = this.filter.clone();
        let dp2 = this.detail_pane.clone();
        let dt2 = this.detail_title.clone();
        let db2 = this.detail_body.clone();
        let di2 = this.detail_icon.clone();
        this.group_list.connect_row_selected(move |_, row| {
            let Some(r) = row else { return };
            let name = r.widget_name();
            title2.set_label(&gettext(name.as_str()));
            let items = apply_source(filter_by_group(&apps2.borrow(), &name), *flt2.borrow());
            populate(
                &list2,
                items,
                &RowCtx {
                    install: &oi2,
                    remove: &orm2,
                    pane: &dp2,
                    title: &dt2,
                    body: &db2,
                    icon: &di2,
                },
                &inst2.borrow(),
            );
        });

        // Source filter dropdown → repopulate the visible group.
        let dp = this.detail_pane.clone();
        detail_close.connect_clicked(move |_| dp.set_visible(false));

        let flt = this.filter.clone();
        let list_f = this.list.clone();
        let apps_f = this.apps.clone();
        let gl_f = this.group_list.clone();
        let oi_f = this.on_install.clone();
        let orm_f = this.on_remove.clone();
        let inst_f = this.installed.clone();
        let dp_f = this.detail_pane.clone();
        let dt_f = this.detail_title.clone();
        let db_f = this.detail_body.clone();
        let di_f = this.detail_icon.clone();
        src_filter.connect_selected_notify(move |dd| {
            *flt.borrow_mut() = dd.selected() as u8;
            if let Some(r) = gl_f.selected_row() {
                let name = r.widget_name();
                let mut items = filter_by_group(&apps_f.borrow(), &name);
                items = apply_source(items, *flt.borrow());
                populate(
                    &list_f,
                    items,
                    &RowCtx {
                        install: &oi_f,
                        remove: &orm_f,
                        pane: &dp_f,
                        title: &dt_f,
                        body: &db_f,
                        icon: &di_f,
                    },
                    &inst_f.borrow(),
                );
            }
        });

        // Search → filter by name/summary across all apps.
        let apps3 = this.apps.clone();
        let list3 = this.list.clone();
        let title3 = this.title.clone();
        let oi3 = on_install;
        let inst3 = this.installed.clone();
        let orm3 = on_remove.clone();
        let flt3 = this.filter.clone();
        let dp3 = this.detail_pane.clone();
        let dt3 = this.detail_title.clone();
        let db3 = this.detail_body.clone();
        let di3 = this.detail_icon.clone();
        this.search.connect_search_changed(move |e| {
            let q = e.text().to_string().to_lowercase();
            if q.is_empty() {
                return;
            }
            title3.set_label(&gettext("Search"));
            let hits: Vec<AppItem> = apply_source(
                apps3
                    .borrow()
                    .iter()
                    .filter(|a| {
                        a.name.to_lowercase().contains(&q)
                            || a.summary.to_lowercase().contains(&q)
                            || a.package.to_lowercase().contains(&q)
                    })
                    .take(200)
                    .cloned()
                    .collect(),
                *flt3.borrow(),
            );
            populate(
                &list3,
                hits,
                &RowCtx {
                    install: &oi3,
                    remove: &orm3,
                    pane: &dp3,
                    title: &dt3,
                    body: &db3,
                    icon: &di3,
                },
                &inst3.borrow(),
            );
        });
        this
    }

    /// Show a status line under the list (install/remove feedback).
    pub fn set_status(&self, t: &str) {
        self.status.set_label(t);
    }

    /// Reload the installed set and repaint the current view —
    /// called after an install/remove transaction finished.
    pub fn refresh_installed(&self) {
        // remember scroll position — repopulating would jump to top
        let v = self.scroll.vadjustment().value();
        *self.installed.borrow_mut() = crate::catalog_data::installed_set();
        // restore after layout settles — immediate set_value clamps
        let adj = self.scroll.vadjustment();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            adj.set_value(v)
        });
        if let Some(row) = self.group_list.selected_row() {
            let name = row.widget_name();
            populate(
                &self.list,
                apply_source(
                    filter_by_group(&self.apps.borrow(), &name),
                    *self.filter.borrow(),
                ),
                &RowCtx {
                    install: &self.on_install,
                    remove: &self.on_remove,
                    pane: &self.detail_pane,
                    title: &self.detail_title,
                    body: &self.detail_body,
                    icon: &self.detail_icon,
                },
                &self.installed.borrow(),
            );
        }
    }

    /// Store the loaded catalogue and refresh the current view.
    /// `installed` marks rows whose package is already present.
    pub fn set_apps(&self, apps: Vec<AppItem>, installed: std::collections::HashSet<String>) {
        *self.installed.borrow_mut() = installed;
        *self.apps.borrow_mut() = apps;
        if let Some(row) = self.group_list.selected_row() {
            let name = row.widget_name();
            populate(
                &self.list,
                apply_source(
                    filter_by_group(&self.apps.borrow(), &name),
                    *self.filter.borrow(),
                ),
                &RowCtx {
                    install: &self.on_install,
                    remove: &self.on_remove,
                    pane: &self.detail_pane,
                    title: &self.detail_title,
                    body: &self.detail_body,
                    icon: &self.detail_icon,
                },
                &self.installed.borrow(),
            );
        }
    }
}

/// Keep components whose categories intersect the chosen group.
/// Apply the source dropdown: 0 all, 1 debian, 2 flatpak.
fn apply_source(items: Vec<AppItem>, f: u8) -> Vec<AppItem> {
    match f {
        1 => items.into_iter().filter(|a| a.source == "debian").collect(),
        2 => items
            .into_iter()
            .filter(|a| a.source == "flatpak")
            .collect(),
        _ => items,
    }
}

fn filter_by_group(apps: &[AppItem], group: &str) -> Vec<AppItem> {
    let cats = GROUPS
        .iter()
        .find(|(l, _)| *l == group)
        .map(|(_, c)| *c)
        .unwrap_or(&[]);
    apps.iter()
        .filter(|a| a.categories.iter().any(|c| cats.contains(&c.as_str())))
        .cloned()
        .collect()
}

/// Resolve an AppItem icon onto an Image widget: an absolute path
/// loads from disk, anything else is tried as a theme icon name.
/// "package-x-generic" is the honest fallback, never a fake icon.
fn set_icon(img: &gtk4::Image, icon: &str, w: &impl IsA<gtk4::Widget>) {
    if icon.starts_with('/') && std::path::Path::new(icon).exists() {
        img.set_from_file(Some(icon));
        return;
    }
    let has = !icon.is_empty() && gtk4::IconTheme::for_display(&w.display()).has_icon(icon);
    img.set_icon_name(Some(if has { icon } else { "package-x-generic" }));
}

/// Render app rows: name + summary + install button.
/// Widgets + callbacks shared by every populate call.
struct RowCtx<'a> {
    install: &'a Rc<dyn Fn((String, String))>,
    remove: &'a Rc<dyn Fn((String, String))>,
    pane: &'a gtk4::Box,
    title: &'a gtk4::Label,
    body: &'a gtk4::Label,
    icon: &'a gtk4::Image,
}

fn populate(
    list: &gtk4::ListBox,
    items: Vec<AppItem>,
    ctx: &RowCtx,
    installed: &std::collections::HashSet<String>,
) {
    while let Some(r) = list.row_at_index(0) {
        list.remove(&r);
    }
    for a in items {
        let name = gtk4::Label::new(Some(&a.name));
        name.set_halign(gtk4::Align::Start);
        let sum = gtk4::Label::new(Some(&a.summary));
        sum.set_halign(gtk4::Align::Start);
        sum.add_css_class("dim-label");
        sum.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let v = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        v.set_margin_start(6);
        v.set_margin_end(6);
        v.set_margin_top(6);
        v.set_margin_bottom(6);
        v.append(&name);
        v.set_hexpand(true);
        v.append(&sum);
        let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let icon = gtk4::Image::new();
        icon.set_pixel_size(32);
        icon.set_valign(gtk4::Align::Center);
        icon.set_margin_start(6);
        set_icon(&icon, &a.icon, &h);
        h.append(&icon);
        h.append(&v);
        let key = if a.source == "flatpak" {
            a.id.replace(".desktop", "")
        } else {
            a.package.clone()
        };
        let is_in = installed.contains(&key) || installed.contains(&a.package);
        let badge = gtk4::Label::new(Some(if a.source == "flatpak" {
            "Flathub"
        } else {
            "Debian"
        }));
        badge.add_css_class("dim-label");
        badge.set_valign(gtk4::Align::Center);
        let label = if is_in {
            gettext("Uninstall")
        } else {
            gettext("Install")
        };
        let btn = gtk4::Button::with_label(&label);
        if is_in {
            btn.add_css_class("destructive-action");
        }
        btn.set_valign(gtk4::Align::Center);
        let cb = if is_in {
            ctx.remove.clone()
        } else {
            ctx.install.clone()
        };
        let pkg = if a.source == "flatpak" {
            a.id.replace(".desktop", "")
        } else {
            a.package.clone()
        };
        let src = a.source.clone();
        btn.connect_clicked(move |_| cb((pkg.clone(), src.clone())));
        let ib = gtk4::Button::from_icon_name("dialog-information-symbolic");
        ib.add_css_class("flat");
        ib.set_valign(gtk4::Align::Center);
        let dt = ctx.title.clone();
        let db = ctx.body.clone();
        let di = ctx.icon.clone();
        let pane = ctx.pane.clone();
        let title2 = a.name.clone();
        let desc = a.desc.clone();
        let icon_str = a.icon.clone();
        ib.connect_clicked(move |_| {
            pane.set_visible(true);
            dt.set_label(&title2);
            db.set_label(&desc);
            set_icon(&di, &icon_str, &pane);
        });
        h.append(&ib);
        h.append(&badge);
        h.append(&btn);
        let row = gtk4::ListBoxRow::new();
        row.set_activatable(false);
        row.set_child(Some(&h));
        list.append(&row);
    }
}
