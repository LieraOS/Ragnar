//! Display settings panel — ported from the old ragnar-settings-gui.
//!
//! Visual monitor arrangement (drag-and-drop canvas with snapping
//! and click selection), mode/primary controls and the GPU section.
//! Backend unchanged: org.lieraos.ragnar.DisplayServices over D-Bus.

mod canvas;
mod controls;
pub(crate) mod dbus_client;
mod gpu;
mod mode_controls;
mod monitor_rect;
mod revert_dialog;

use gtk4::Widget;

/// Build the display panel widget.
pub fn build() -> Widget {
    canvas::create_page()
}
