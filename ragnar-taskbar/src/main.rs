// Ragnar taskbar — a Wayland-native GTK4 panel for LieraOS.
//
// One process owns one or more layer-shell bar surfaces. Each bar is a
// three-zone container (start / centre / end) holding applets — menu,
// task list, tray, clock — described by a per-bar config. Bars are
// reconfigured live: geometry, edge, width and alignment come from the
// user-editable config file and can be changed through the bar's own
// right-click properties dialog.
mod applets;
mod bar;
mod bar_anim;
mod bars;
mod config;
mod i18n;
mod menu;
mod menucfg;
mod settings;
mod toplevel;
mod tray;
mod workspace;

use gtk4::prelude::*;
use gtk4::Application;

fn main() {
    // gettext before any widget exists — strings are marked at
    // construction time and never re-evaluated.
    i18n::init();
    // Run from $HOME, not wherever the parent launched us: apps
    // started from the start menu inherit our CWD (a terminal would
    // open inside the taskbar's own directory), and a stray CWD on a
    // removable mount keeps it busy forever.
    if let Err(e) = std::env::set_current_dir(gtk4::glib::home_dir()) {
        eprintln!("[main] set_current_dir home failed: {e}");
    }

    let app = Application::builder()
        .application_id("org.lieraos.ragnar.Taskbar")
        .flags(gtk4::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_command_line(|app, cl| {
        // "--settings" on a forwarded (remote) invocation opens the
        // settings window on the running instance — used by the
        // ragnar-settings Taskbar tile. Checked on the command line,
        // not the app: is_remote() on the app is always false here.
        if cl.is_remote()
            && cl
                .arguments()
                .iter()
                .any(|a| a.to_string_lossy() == "--settings")
        {
            if let Some(w) = app.windows().first() {
                crate::settings::open(w);
            }
        } else {
            // Dark preference from our own settings, not the portal —
            // a late/missing appearance answer left widgets in the
            // light fallback theme.
            if let Some(s) = gtk4::Settings::default() {
                s.set_gtk_application_prefer_dark_theme(true);
            }
            applets::init_css();
            let (store, rx) = toplevel::start();
            let (tray, tray_rx) = tray::start();
            let (ws_store, ws_rx) = workspace::start();
            bars::init(app, store, rx, tray, tray_rx, ws_store, ws_rx);
        }
        0
    });
    app.run();
}
pub mod extcap;
