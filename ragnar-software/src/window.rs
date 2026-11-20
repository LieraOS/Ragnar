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
            // detail title: name part of a PackageKit id ("name;…")
            // or the ref id of "flatpak:origin:kind/org.id/…"
            *dn.borrow_mut() = if let Some(rest) = id.strip_prefix("flatpak:") {
                rest.split_once(':')
                    .and_then(|(_, r)| r.split('/').nth(1))
                    .unwrap_or(rest)
                    .to_string()
            } else {
                id.split(';').next().unwrap_or("").to_string()
            };
            backend::detail(id, tx_detail.clone());
        },
    );
    stack.add_named(&updates.widget, Some("updates"));

    // Software catalogue page.
    let tx_cat = tx.clone();
    let tx_rm = tx.clone();
    let catalog = CatalogPage::new(
        move |(pkg, src)| backend::install_pkg_any(pkg, src, tx_cat.clone()),
        move |(pkg, src)| backend::uninstall_pkg_any(pkg, src, tx_rm.clone()),
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
        ("welcome", gettext("Welcome")),
        ("updates", gettext("Updates")),
        ("software", gettext("Software")),
        ("history", gettext("History")),
        ("settings", gettext("Settings")),
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
    // Lazy progress dialog: opened on InstallStart / the first
    // progress signal from any transaction, updated live, closed on
    // InstallDone. Tuple: dialog, headline, bar, log buffer.
    let dlg_state: Rc<
        RefCell<Option<(adw::Dialog, gtk4::Label, gtk4::ProgressBar, gtk4::TextBuffer)>>,
    > = Rc::new(RefCell::new(None));
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
                    catalog.set_status(&format!("{} {p} %", gettext("Installing …")));
                    let mut d = dlg_state.borrow_mut();
                    if d.is_none() {
                        open_dlg(&mut d, &win2);
                    }
                    if let Some((_, lbl, bar, _)) = d.as_ref() {
                        lbl.set_label(&gettext("Installing …"));
                        bar.set_fraction(p as f64 / 100.0);
                    }
                }
                Msg::InstallPhase(i, n) => {
                    let line = format!("{}: {n}", crate::packagekit::phase_label(i));
                    updates.borrow().set_progress_phase(i, &n);
                    catalog.set_status(&line);
                    let mut d = dlg_state.borrow_mut();
                    // No percentage may ever arrive for a transaction
                    // (PackageKit emits none during many ops) — the
                    // first phase signal must already open the dialog.
                    if d.is_none() {
                        open_dlg(&mut d, &win2);
                    }
                    if let Some((_, lbl, bar, buf)) = d.as_ref() {
                        lbl.set_label(&line);
                        bar.pulse();
                        buf.insert_at_cursor(&(line.clone() + "\n"));
                    }
                }
                Msg::Deps(ids, deps) => {
                    // Name the actual selection first — the user must
                    // see what they checked, not just the side effects.
                    let names: Vec<String> = ids
                        .iter()
                        .map(|i| {
                            if let Some(rest) = i.strip_prefix("flatpak:") {
                                rest.split('/').nth(1).unwrap_or(rest).to_string()
                            } else {
                                i.split(';').next().unwrap_or(i).to_string()
                            }
                        })
                        .collect();
                    let mut txt =
                        gettext("The following updates will be installed:") + "\n" + &names.join(", ");
                    if !deps.is_empty() {
                        txt += "\n\n";
                        txt += &gettext("The following will also be installed:");
                        txt += "\n";
                        txt += &deps.join(", ");
                    }
                    let dlg = adw::AlertDialog::builder()
                        .heading(gettext("Install updates?"))
                        .body(txt)
                        .build();
                    dlg.add_response("cancel", &gettext("Cancel"));
                    dlg.add_response("ok", &gettext("Install"));
                    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
                    let ti = tx_pump.clone();
                    dlg.connect_response(None, move |_, r| {
                        if r == "ok" {
                            backend::install(ids.clone(), ti.clone());
                        }
                    });
                    dlg.present(Some(&win));
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
                    if let Some((dg, _, _, _)) = dlg_state.borrow_mut().take() {
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

/// Open the shared transaction progress dialog: headline + bar on
/// top, and — Synaptic-style — an expandable Details section that
/// logs every package phase the backend reports (downloading,
/// installing, removing …).
fn open_dlg(
    d: &mut Option<(adw::Dialog, gtk4::Label, gtk4::ProgressBar, gtk4::TextBuffer)>,
    win: &adw::ApplicationWindow,
) {
    let lbl = gtk4::Label::new(Some(&gettext("Working …")));
    lbl.set_halign(gtk4::Align::Start);
    let bar = gtk4::ProgressBar::new();
    bar.set_pulse_step(0.1);
    bar.pulse();
    let log = gtk4::TextView::new();
    log.set_editable(false);
    log.set_cursor_visible(false);
    log.set_monospace(true);
    log.set_wrap_mode(gtk4::WrapMode::WordChar);
    let lscroll = gtk4::ScrolledWindow::builder()
        .child(&log)
        .min_content_height(140)
        .propagate_natural_height(true)
        .build();
    let exp = gtk4::Expander::new(Some(&gettext("Details")));
    exp.set_child(Some(&lscroll));
    let bx = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    bx.set_margin_start(20);
    bx.set_margin_end(20);
    bx.set_margin_top(20);
    bx.set_margin_bottom(20);
    bx.set_size_request(420, -1);
    bx.append(&lbl);
    bx.append(&bar);
    bx.append(&exp);
    let dg = adw::Dialog::builder()
        .title(gettext("Package management"))
        .child(&bx)
        .build();
    dg.present(Some(win));
    *d = Some((dg, lbl, bar, log.buffer()));
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
    let btn = gtk4::Button::with_label(&gettext("Install"));
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
        .title(gettext("Install package"))
        .default_width(560)
        .default_height(220)
        .content(&tb)
        .build();
    // Button morphs: "Install" → "Close" when the tx finishes.
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
                    status.set_label(&gettext("Transaction running …"));
                }
                Msg::InstallProgress(p) => {
                    bar.set_visible(true);
                    bar.set_fraction(p as f64 / 100.0);
                }
                Msg::InstallPhase(i, n) => {
                    bar.set_visible(true);
                    bar.pulse();
                    status.set_label(&format!(
                        "{}: {n}",
                        crate::packagekit::phase_label(i)
                    ));
                }
                Msg::InstallDone(r) => {
                    bar.set_visible(false);
                    done.set(true);
                    btn.set_label(&gettext("Close"));
                    btn.set_sensitive(true);
                    status.set_label(&match r {
                        Ok(()) => gettext("Installation finished."),
                        Err(e) => gettext("Installation failed: %s").replace("%s", &e),
                    });
                }
                _ => {}
            }
        }
        glib::ControlFlow::Continue
    });
}
