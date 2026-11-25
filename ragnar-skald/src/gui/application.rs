// Copyright (C) Jörg Schröder
//! GTK4 Application
//!
//! Application lifecycle: starts the PipeWire backend on activation,
//! builds the main window, or shows an error dialog if the backend
//! thread cannot be spawned.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{AlertDialog, Application};
use tracing::{debug, error, info};

use crate::gui::window::SkaldWindow;
use crate::pipewire::backend::PwBackend;

/// Skald GTK4 Application
pub struct SkaldApplication {
    app: Application,
}

impl SkaldApplication {
    /// Create a new application.
    pub fn new(app_id: &str) -> Self {
        let app = Application::builder().application_id(app_id).build();
        debug!("Application created with ID: {app_id}");
        Self { app }
    }

    /// Run the GTK main loop.
    pub fn run(&self) {
        self.app.connect_activate(Self::build_ui);
        self.app.run();
    }

    /// Activation: spawn backend + build window, or report the failure.
    fn build_ui(app: &Application) {
        match PwBackend::start() {
            Ok(backend) => {
                let window = SkaldWindow::new(app, Rc::new(backend));
                window.present();
                info!("Main window presented");
            }
            Err(e) => {
                error!("Failed to start PipeWire backend: {e:#}");
                AlertDialog::builder()
                    .message("PipeWire-Backend konnte nicht gestartet werden")
                    .detail(format!("{e:#}"))
                    .modal(true)
                    .build()
                    .show(None::<&gtk4::Window>);
            }
        }
    }
}
