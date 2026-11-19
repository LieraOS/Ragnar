//! Application lifecycle for Ragnar Settings.

use libadwaita as adw;

use adw::prelude::*;
use adw::Application;
use std::cell::RefCell;
use std::rc::Rc;

use ragnar_settings::ui::SettingsWindow;

/// Application id on the session bus (and used by the desktop file).
pub const APP_ID: &str = "org.lieraos.ragnar.Settings";

/// Value following `--panel` on the command line, if any.
fn panel_arg(args: &[String]) -> Option<String> {
    let at = args.iter().position(|a| a == "--panel")?;
    args.get(at + 1).cloned()
}

/// Build and run the libadwaita application.
///
/// Single instance: a second `ragnar-settings-gui --panel <id>`
/// is forwarded to the running one, which jumps to that panel —
/// this is how the desktop menu links straight to a settings area.
pub fn run() -> gtk4::glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let window: Rc<RefCell<Option<Rc<SettingsWindow>>>> = Rc::default();
    app.connect_command_line(move |app, cl| {
        let args: Vec<String> = cl
            .arguments()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let win = window
            .borrow_mut()
            .get_or_insert_with(|| SettingsWindow::new(app))
            .clone();
        if let Some(id) = panel_arg(&args) {
            win.open_panel(&id);
        }
        win.present();
        0
    });
    app.run()
}
