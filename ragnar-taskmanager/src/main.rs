// SPDX-License-Identifier: GPL-3.0-or-later
//! Ragnar Task Manager — processes, performance and services for the
//! Ragnar desktop. Rust + GTK4, Wayland-native. Deliberately mirrors
//! the classic task-manager layout: menu bar, horizontal tabs,
//! status bar. Data comes from procfs/sysfs and systemd D-Bus.

mod i18n;
mod procfs;
mod sysstat;
mod proccat;
mod tiles;
mod util;
mod procdetail;
mod processes;
mod performance;
mod services;
mod startup;
mod info;
mod users;

use gettextrs::gettext as tr;
use gtk4::prelude::*;
use libadwaita::prelude::*;

const APP_ID: &str = "org.lieraos.ragnar.TaskManager";

/// Application entry: gettext, then run the app.
fn main() {
    i18n::init();
    let app = gtk4::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        let win = gtk4::ApplicationWindow::builder()
            .application(app)
            .title(&tr("Ragnar Task Manager"))
            .default_width(1060).default_height(680).build();
        build_ui(app, &win);
        win.present();
    });
    app.run();
}

/// Whole window: menu bar, tab bar, page stack, status bar.
fn build_ui(app: &gtk4::Application, win: &gtk4::ApplicationWindow) {
    // Category row tints — same hues as the Qt original.
    let css = gtk4::CssProvider::new();
    css.load_from_data(
        ".cat-app{background:rgba(52,152,219,.2)}
         .cat-sub{background:rgba(52,152,219,.08)}
         .cat-bg{background:rgba(149,165,166,.15)}
         .cat-sys{background:rgba(180,120,60,.15)}",
    );
    if let Some(d) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &d, &css, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&menu_bar(app, win));
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // Horizontal tab bar — QTabWidget look.
    let stack = gtk4::Stack::new();
    stack.set_vexpand(true);
    let tabs = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    tabs.set_margin_start(6); tabs.set_margin_top(4); tabs.set_margin_bottom(4);
    let mut first: Option<gtk4::ToggleButton> = None;
    for (id, label) in [
        ("info", tr("Info")), ("performance", tr("Performance")),
        ("processes", tr("Processes")), ("startup", tr("Startup")),
        ("services", tr("Services")), ("users", tr("Users")),
    ] {
        let b = gtk4::ToggleButton::with_label(&label);
        if let Some(f) = &first { b.set_group(Some(f)); } else {
            b.set_active(true); first = Some(b.clone());
        }
        let stack = stack.clone();
        b.connect_toggled(move |b| if b.is_active() { stack.set_visible_child_name(id) });
        tabs.append(&b);
    }
    root.append(&tabs);
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // Pages — processes gets a setter so Users can jump+filter.
    let (proc_page, set_user_filter) = processes::page();
    stack.add_named(&info::page(), Some("info"));
    stack.add_named(&performance::page(), Some("performance"));
    stack.add_named(&proc_page, Some("processes"));
    stack.add_named(&startup::page(), Some("startup"));
    stack.add_named(&services::page(), Some("services"));
    stack.add_named(&users::page(set_user_filter, stack.clone()), Some("users"));
    root.append(&stack);
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    root.append(&status_bar());

    win.set_child(Some(&root));
}

/// Classic menu bar: File/Exit, Edit/Settings, Help/About.
fn menu_bar(app: &gtk4::Application, win: &gtk4::ApplicationWindow) -> gtk4::Widget {
    let file = gio::Menu::new();
    file.append(Some(&tr("Exit")), Some("app.quit"));
    let edit = gio::Menu::new();
    edit.append(Some(&tr("Settings")), Some("app.settings"));
    let help = gio::Menu::new();
    help.append(Some(&tr("About")), Some("app.about"));

    let model = gio::Menu::new();
    model.append_submenu(Some(&tr("File")), &file);
    model.append_submenu(Some(&tr("Edit")), &edit);
    model.append_submenu(Some(&tr("Help")), &help);

    let quit = gio::SimpleAction::new("quit", None);
    let w = win.clone();
    quit.connect_activate(move |_, _| w.close());
    app.add_action(&quit);

    let about = gio::SimpleAction::new("about", None);
    let w = win.clone();
    about.connect_activate(move |_, _| show_about(&w));
    app.add_action(&about);

    let settings = gio::SimpleAction::new("settings", None);
    let w = win.clone();
    settings.connect_activate(move |_, _| show_settings(&w));
    app.add_action(&settings);

    let bar = gtk4::PopoverMenuBar::from_model(Some(&model));
    bar.upcast()
}

/// About dialog — keeps the attribution notice from the original.
fn show_about(win: &gtk4::ApplicationWindow) {
    let d = libadwaita::AboutDialog::builder()
        .application_name("Ragnar Task Manager").version("0.1.0")
        .developer_name("Jörg Schröder")
        .copyright("Copyright © 2025 Jörg Schröder")
        .license_type(gtk4::License::Gpl30)
        .website("https://www.lieraos.com")
        .comments(&tr("A modern, user-friendly system task manager\n\
                       designed for efficient system monitoring."))
        .build();
    d.present(Some(win));
}

/// Settings dialog — refresh interval is the only option for now.
fn show_settings(win: &gtk4::ApplicationWindow) {
    let d = gtk4::Window::builder().transient_for(win).modal(true)
        .title(&tr("Settings")).default_width(320).build();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    b.set_margin_top(16); b.set_margin_bottom(16);
    b.set_margin_start(16); b.set_margin_end(16);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::builder().label(&tr("Refresh interval (s):"))
        .hexpand(true).halign(gtk4::Align::Start).build());
    let spin = gtk4::SpinButton::with_range(0.5, 10.0, 0.5);
    spin.set_value(crate::interval_secs());
    spin.connect_value_changed(|s| crate::set_interval(s.value()));
    row.append(&spin);
    b.append(&row);
    let close = gtk4::Button::with_label(&tr("Close"));
    let d2 = d.clone();
    close.connect_clicked(move |_| d2.close());
    b.append(&close);
    d.set_child(Some(&b));
    d.present();
}

/// Refresh interval shared by all tabs (seconds, default 1.0).
static INTERVAL_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);

/// Current refresh interval in seconds.
pub fn interval_secs() -> f64 {
    INTERVAL_MS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
}

/// Store the refresh interval (seconds).
pub fn set_interval(s: f64) {
    INTERVAL_MS.store((s * 1000.0) as u64, std::sync::atomic::Ordering::Relaxed);
}

/// Current refresh interval as a Duration for timeouts.
pub fn interval() -> std::time::Duration {
    std::time::Duration::from_millis(INTERVAL_MS.load(std::sync::atomic::Ordering::Relaxed))
}

/// Bottom status bar: CPU% | Memory% | process count, ticking.
fn status_bar() -> gtk4::Widget {
    let l = gtk4::Label::new(Some(""));
    let l2 = l.clone();
    l.set_halign(gtk4::Align::Start);
    l.set_margin_start(8); l.set_margin_bottom(4); l.set_margin_top(4);
    let sampler = std::rc::Rc::new(std::cell::RefCell::new(sysstat::CpuSampler::default()));
    let table = std::rc::Rc::new(std::cell::RefCell::new(procfs::ProcessTable::new()));
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let l = l2.clone();
        let cores = sampler.borrow_mut().sample();
        let cpu = if cores.is_empty() { 0.0 }
            else { cores.iter().sum::<f32>() / cores.len() as f32 * 100.0 };
        let m = sysstat::meminfo();
        let mem = if m.total > 0 {
            (m.total - m.avail) as f64 / m.total as f64 * 100.0
        } else { 0.0 };
        table.borrow_mut().refresh();
        let n = table.borrow().rows.len();
        l.set_text(&format!("CPU: {cpu:.1}% | Memory: {mem:.1}% | {}: {n}", tr("Processes")));
        glib::ControlFlow::Continue
    });
    l.upcast()
}
