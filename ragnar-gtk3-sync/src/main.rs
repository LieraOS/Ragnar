// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-gtk3-sync — GTK3 settings.ini synchronisation daemon
//
// Rust replacement for the live part of the legacy daemon's
// SettingsManager: GTK3 does not read GSettings, so theme changes
// written to org.gnome.desktop.interface would never reach GTK3
// applications without a bridge that mirrors them into
// ~/.config/gtk-3.0/settings.ini. Also keeps the Ragnar panel's
// dark-theme key and the freedesktop color-scheme key consistent
// when a writer only updates one side.

mod color_scheme;
mod gtk_ini;

fn main() {
    // Without the interface schema there is nothing to mirror —
    // exiting cleanly is correct (autostart won't loop-spawn).
    if !gtk_ini::available() {
        eprintln!("ragnar-gtk3-sync: interface schema missing, exiting");
        return;
    }

    // Apply current state once at startup, then on every change —
    // same contract as the legacy SettingsManager constructor.
    gtk_ini::sync();
    color_scheme::sync();

    gtk_ini::watch();
    color_scheme::watch();

    glib::MainLoop::new(None, false).run();
}
