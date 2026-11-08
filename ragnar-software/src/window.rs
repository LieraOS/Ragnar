// SPDX-License-Identifier: GPL-3.0-or-later
//! Main window: sidebar navigation (Willkommen / Updates) over a
//! stacked content area, wired to the backend message channel.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::backend::{self, Msg};
use crate::catalog::CatalogPage;
use crate::history::HistoryPage;
use crate::settings::SettingsPage;
use crate::updates::UpdatesPage;
use crate::welcome::WelcomePage;

/// Build the application window and start the backend pump.
pub fn build(app: &adw::Application) {
    let (tx, rx) = mpsc::channel::<Msg>();

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

    let welcome = WelcomePage::new();
    stack.add_named(
        &welcome.widget.clone().upcast::<gtk4::Widget>(),
        Some("welcome"),
    );

    // Install action → backend worker.
    let tx_install = tx.clone();
    let tx_detail = tx.clone();
    let detail_name = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let dn = detail_name.clone();
    let last_install = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let li = last_install.clone();
    let updates = UpdatesPage::new(
        move |ids| {
            *li.borrow_mut() = ids
                .iter()
                .map(|i| i.split(';').next().unwrap_or("").to_string())
                .collect();
            backend::deps_preview(ids, tx_install.clone());
        },
        move |id| {
            *dn.borrow_mut() = id.split(';').next().unwrap_or("").to_string();
            backend::detail(id, tx_detail.clone());
        },
    );
    stack.add_named(&updates.widget, Some("updates"));

    // Software catalogue page.
    let tx_cat = tx.clone();
    let tx_rm = tx.clone();
    let tx_det = tx.clone();
    let catalog = CatalogPage::new(
        move |(pkg, src)| backend::install_pkg_any(pkg, src, tx_cat.clone()),
        move |(pkg, src)| backend::uninstall_pkg_any(pkg, src, tx_rm.clone()),
        move |(pkg, src)| backend::app_detail(pkg, src, tx_det.clone()),
    );
    stack.add_named(
        &catalog.widget.clone().upcast::<gtk4::Widget>(),
        Some("software"),
    );

    // Transaction history page.
    let history = HistoryPage::new();
    stack.add_named(
        &history.widget.clone().upcast::<gtk4::Widget>(),
        Some("history"),
    );

    // Settings page (sources management).
    let settings = SettingsPage::new();
    stack.add_named(
        &settings.widget.clone().upcast::<gtk4::Widget>(),
        Some("settings"),
    );

    // Sidebar.
    let sidebar_list = gtk4::ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    for (id, label) in [
        ("welcome", gettext("Willkommen")),
        ("updates", gettext("Updates")),
        ("software", gettext("Software")),
        ("history", gettext("Verlauf")),
        ("settings", gettext("Einstellungen")),
    ] {
        let row = gtk4::ListBoxRow::new();
        row.set_widget_name(id);
        let lbl = gtk4::Label::new(Some(&label));
        lbl.set_halign(gtk4::Align::Start);
        lbl.set_margin_start(12);
        lbl.set_margin_end(12);
        lbl.set_margin_top(8);
        lbl.set_margin_bottom(8);
        row.set_child(Some(&lbl));
        sidebar_list.append(&row);
    }
    let stack2 = stack.clone();
    sidebar_list.connect_row_selected(move |_, row| {
        if let Some(r) = row {
            stack2.set_visible_child_name(r.widget_name().as_str());
        }
    });
    sidebar_list.select_row(sidebar_list.row_at_index(0).as_ref());

    let sidebar_page = adw::NavigationPage::new(&sidebar_list, &gettext("Ragnar Software"));
    let content_page = adw::NavigationPage::new(&stack, &gettext("Ragnar Software"));
    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(220.0)
        .max_sidebar_width(280.0)
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&split));

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(gettext("Ragnar Software"))
        .default_width(860)
        .default_height(560)
        .content(&toolbar)
        .build();
    win.present();

    // Initial background check.
    let welcome = Rc::new(welcome);
    let updates = Rc::new(RefCell::new(updates));
    let catalog = Rc::new(catalog);
    let history = Rc::new(history);
    let tx: mpsc::Sender<Msg> = tx.clone();
    backend::fetch(tx.clone());
    backend::load_catalog(tx.clone());
    backend::history(tx.clone());
    let welcome_ref = welcome.clone();
    // Lazy progress dialog: opened on the first progress signal from
    // any transaction, updated live, closed on InstallDone.
    let dlg_state: Rc<RefCell<Option<(adw::Dialog, gtk4::Label, gtk4::ProgressBar)>>> =
        Rc::new(RefCell::new(None));
    let win2 = win.clone();

    // Pump backend messages into the widgets (poll the worker
    // channel — std mpsc has no glib source of its own).
    let tx_pump = tx.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Updates(list) => {
                    welcome_ref.set_count(list.len());
                    updates.borrow().set_items(&list);
                }
                Msg::FetchFailed(e) => welcome_ref.set_error(&e),
                Msg::InstallStart => {
                    let mut d = dlg_state.borrow_mut();
                    if d.is_none() {
                        open_dlg(&mut d, &win2);
                    }
                }
                Msg::InstallProgress(p) => {
                    updates.borrow().set_progress(p);
                    catalog.set_status(&format!("Installiere … {p} %"));
                    let mut d = dlg_state.borrow_mut();
                    if d.is_none() {
                        open_dlg(&mut d, &win2);
                    }
                    if let Some((_, lbl, bar)) = d.as_ref() {
                        lbl.set_label(&gettext("Installiere …"));
                        bar.set_fraction(p as f64 / 100.0);
                    }
                }
                Msg::InstallName(n) => {
                    updates.borrow().set_progress_name(&n);
                    catalog.set_status(&format!("Paket: {n}"));
                    if let Some((_, lbl, bar)) = dlg_state.borrow().as_ref() {
                        lbl.set_label(&format!("Paket: {n}"));
                        bar.pulse();
                    } else {
                        // no percentage yet — open dialog via the
                        // progress path on next tick
                    }
                }
                Msg::Deps(ids, deps) => {
                    let txt = if deps.is_empty() {
                        "Keine zusätzlichen Abhängigkeiten.".to_string()
                    } else {
                        format!("Zusätzlich werden installiert:\n{}", deps.join(", "))
                    };
                    let dlg = adw::AlertDialog::builder()
                        .heading(gettext("Updates installieren?"))
                        .body(txt)
                        .build();
                    dlg.add_response("cancel", &gettext("Abbrechen"));
                    dlg.add_response("ok", &gettext("Installieren"));
                    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
                    let ti = tx_pump.clone();
                    dlg.connect_response(None, move |_, r| {
                        if r == "ok" {
                            backend::install(ids.clone(), ti.clone());
                        }
                    });
                    dlg.present(Some(&win));
                }
                Msg::AppDetail(name, body) => {
                    eprintln!("[dbg] AppDetail {} ({}b)", name, body.len());
                    catalog.show_app_detail(&name, &body);
                }
                Msg::Detail(d) => {
                    let name = detail_name.borrow().clone();
                    updates.borrow().show_detail(&name, "", &d);
                }
                Msg::DetailFailed(e) => updates.borrow().show_detail_error(&e),
                Msg::Apps(apps, inst) => catalog.set_apps(apps, inst),
                Msg::History(h) => history.set_entries(&h),
                Msg::HistoryFailed(e) => history.set_error(&e),
                Msg::InstallDone(res) => {
                    // close the transaction dialog — non-blocking by now
                    if let Some((dg, _, _)) = dlg_state.borrow_mut().take() {
                        dg.close();
                    }
                    catalog.refresh_installed();
                    match &res {
                        Ok(()) => updates.borrow().install_done(true, ""),
                        Err(e) => updates.borrow().install_done(false, e),
                    }
                    // Re-check what remains after a successful install.
                    welcome_ref.set_checking();
                    backend::fetch(tx_pump.clone());
                    // new transactions land in the history too
                    backend::history(tx_pump.clone());
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

/// Open the shared transaction progress dialog.
fn open_dlg(
    d: &mut Option<(adw::Dialog, gtk4::Label, gtk4::ProgressBar)>,
    win: &adw::ApplicationWindow,
) {
    let lbl = gtk4::Label::new(Some(&gettext("Arbeite …")));
    let bar = gtk4::ProgressBar::new();
    bar.set_pulse_step(0.1);
    bar.pulse();
    let bx = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    bx.set_margin_start(20);
    bx.set_margin_end(20);
    bx.set_margin_top(20);
    bx.set_margin_bottom(20);
    bx.set_size_request(360, -1);
    bx.append(&lbl);
    bx.append(&bar);
    let dg = adw::Dialog::builder()
        .title(gettext("Paketverwaltung"))
        .child(&bx)
        .build();
    dg.present(Some(win));
    *d = Some((dg, lbl, bar));
}

/// Minimal window for a local package file (.deb/.flatpakref):
/// inline progress — a separate modal dialog would hide the
/// polkit prompt behind it.
pub fn build_install(app: &adw::Application, path: String) {
    let (tx, rx) = mpsc::channel::<Msg>();
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let lbl = gtk4::Label::new(Some(&name));
    lbl.add_css_class("title-3");
    let info = gtk4::Label::new(Some(&path));
    info.add_css_class("dim-label");
    info.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    let bar = gtk4::ProgressBar::new();
    bar.set_visible(false);
    let status = gtk4::Label::new(None);
    status.add_css_class("dim-label");
    let btn = gtk4::Button::with_label(&gettext("Installieren"));
    btn.add_css_class("suggested-action");
    let bx = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    bx.set_margin_start(24);
    bx.set_margin_end(24);
    bx.set_margin_top(24);
    bx.set_margin_bottom(24);
    bx.append(&lbl);
    bx.append(&info);
    bx.append(&bar);
    bx.append(&status);
    bx.append(&btn);
    let tb = adw::ToolbarView::new();
    tb.add_top_bar(&adw::HeaderBar::new());
    tb.set_content(Some(&bx));
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(&gettext("Paket installieren"))
        .default_width(560)
        .default_height(220)
        .content(&tb)
        .build();
    // Button morphs: "Installieren" → "Beenden" when the tx finishes.
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let txc = tx.clone();
    let p = path.clone();
    let done2 = done.clone();
    let win2 = win.clone();
    btn.connect_clicked(move |_| {
        if done2.get() {
            win2.close();
        } else {
            backend::install_local(p.clone(), txc.clone());
        }
    });
    win.present();
    glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::InstallStart => {
                    bar.set_visible(true);
                    bar.pulse();
                    btn.set_sensitive(false);
                    status.set_label(&gettext("Transaktion läuft …"));
                }
                Msg::InstallProgress(p) => {
                    bar.set_visible(true);
                    bar.set_fraction(p as f64 / 100.0);
                }
                Msg::InstallName(n) => {
                    bar.set_visible(true);
                    bar.pulse();
                    status.set_label(&format!("Paket: {n}"));
                }
                Msg::InstallDone(r) => {
                    bar.set_visible(false);
                    done.set(true);
                    btn.set_label(&gettext("Beenden"));
                    btn.set_sensitive(true);
                    status.set_label(&match r {
                        Ok(()) => gettext("Installation abgeschlossen."),
                        Err(e) => gettext("Installation fehlgeschlagen: %s").replace("%s", &e),
                    });
                }
                _ => {}
            }
        }
        glib::ControlFlow::Continue
    });
}
