// Applet zone builders. Each applet is a self-contained widget dropped
// into a zone slot; new applets register a builder here and a config
// key to place them.
pub mod clock;
pub mod clock_settings;
pub mod cmd;
pub mod menu;
pub mod notify;
pub mod preview;
pub mod sensor;
pub mod status;
pub mod tasklist;
pub mod tray;
pub mod volume_popup;
pub mod workspace;

use gtk4::gdk::Display;
use gtk4::{CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};

/// Taskbar styling: solid bar, no window chrome, compact applets.
pub fn init_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        ".ragnar-taskbar { padding: 0 8px; } .bar-transparent { background: transparent; } .has-notes { color: #3584e4; } .ragnar-taskbar.vertical { padding: 8px 0; } \
         .ragnar-taskbar button { min-height: 0; padding: 0 8px; background: transparent; border-color: transparent; box-shadow: none; text-shadow: none; } \
         .ragnar-taskbar.vertical button { padding: 8px 0; } \
         .ragnar-taskbar button:hover { background: rgba(255,255,255,0.12); border-radius: 5px; } \
         .ragnar-taskbar button:active { background: rgba(255,255,255,0.18); border-radius: 5px; } \
         .ragnar-taskbar button.active { background: rgba(255,255,255,0.2); border-radius: 5px; } \
         .ragnar-taskbar .ws-button { min-width: 30px; padding: 0 6px; } \
         .ragnar-taskbar.vertical .ws-button { min-width: 0; min-height: 30px; padding: 6px 0; } \
         .ragnar-taskbar .ws-button.urgent { color: #e01b24; font-weight: bold; } \
         .menu-overlay { background: transparent; } \
         .menu-box { background: @theme_bg_color; border-radius: 6px; padding: 4px; box-shadow: 0 2px 8px rgba(0,0,0,0.35); } \
         .menu-box button { min-height: 0; padding: 4px 12px; } \
         .settings-card { background: @theme_bg_color; border-radius: 8px; padding: 10px; } \
         .settings-card.selected { background: alpha(@theme_selected_bg_color, 0.25); outline: 1px solid alpha(@theme_selected_bg_color, 0.6); } \
         .settings-card .card-title { font-weight: bold; } \
         .settings-add { padding: 10px; border-style: dashed; border-width: 1px; border-radius: 8px; } \
         .screen-mock { background: alpha(@theme_fg_color, 0.08); border: 1px solid alpha(@theme_fg_color, 0.25); border-radius: 8px; } \
         .edge-btn { border-radius: 15px; } \
         .edge-btn:checked { background: @theme_selected_bg_color; color: @theme_selected_fg_color; } \
         .layout-preview { background: alpha(@theme_fg_color, 0.05); border-radius: 6px; padding: 4px; } \
         .layout-chip { min-height: 0; padding: 2px 10px; }",
    );
    if let Some(d) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &d,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
