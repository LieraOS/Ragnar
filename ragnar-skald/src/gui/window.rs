//! Main Window
//!
//! Main application window with the Ausgabe/Eingabe tabs. The window
//! routes `BackendEvent`s into both panels via a glib timer that drains
//! the worker's event channel.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box, Stack, StackSwitcher};
use tracing::{debug, warn};

use crate::config::{AppSettings, ConfigPersistence};
use crate::gui::input_panel::InputPanel;
use crate::gui::output_panel::OutputPanel;
use crate::pipewire::backend::PwBackend;

/// How often the GUI drains pending backend events.
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Main application window
pub struct SkaldWindow {
    window: ApplicationWindow,
}

impl SkaldWindow {
    /// Create the main window.
    ///
    /// `backend` is shared with the panels, which send commands into the
    /// PipeWire worker; a glib timer forwards `BackendEvent`s back.
    pub fn new(app: &Application, backend: Rc<PwBackend>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Skald")
            .default_width(600)
            .default_height(420)
            .build();

        let main_box = Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_start(10)
            .margin_end(10)
            .margin_top(10)
            .margin_bottom(10)
            .build();

        let stack = Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideLeftRight)
            .build();

        // Shared settings: the effects widget reads its saved state
        // from here and writes changes back (debounced).
        let persist = Rc::new(ConfigPersistence::new().unwrap_or_default());
        let settings = match persist.load() {
            Ok(s) => Rc::new(RefCell::new(s)),
            Err(e) => {
                warn!("Settings load failed, using defaults: {e:#}");
                Rc::new(RefCell::new(AppSettings::new()))
            }
        };

        let output_panel = OutputPanel::new(
            Rc::clone(&backend),
            Rc::clone(&settings),
            Rc::clone(&persist),
        );
        let input_panel = InputPanel::new(Rc::clone(&backend), settings, Rc::clone(&persist));

        stack.add_child(output_panel.widget()).set_title("Ausgabe");
        stack.add_child(input_panel.widget()).set_title("Eingabe");

        let switcher = StackSwitcher::builder().stack(&stack).build();
        main_box.append(&switcher);
        main_box.append(&stack);
        window.set_child(Some(&main_box));

        // Re-apply persisted effect state — the backend defers the
        // chain load internally until the default metadata is bound.
        output_panel.restore_effects();
        input_panel.restore_effects();

        // Drain backend events on the GTK main context. The worker only
        // sends through a std channel, so a short timer is the bridge.
        glib::timeout_add_local(EVENT_POLL_INTERVAL, move || loop {
            match backend.try_recv_event() {
                Ok(event) => {
                    output_panel.apply_event(&event);
                    input_panel.apply_event(&event);
                }
                Err(_) => return glib::ControlFlow::Continue,
            }
        });

        debug!("Main window created");
        Self { window }
    }

    /// Present the window
    pub fn present(&self) {
        self.window.present();
    }
}
