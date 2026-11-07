//! Application bootstrap: libadwaita app, stylesheet hook, main window.

use libadwaita as adw;

use adw::prelude::*;
use adw::Application;

use crate::ui::window::MainWindow;

/// Well-known application id used for single-instance behaviour.
const APP_ID: &str = "org.lieraos.ragnar.Heimdall";

/// Create the application, wire activation, and run the main loop.
pub fn run() -> i32 {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        adw::init().ok();
        MainWindow::new(app).present();
    });
    app.run_with_args::<&str>(&[]).into()
}
