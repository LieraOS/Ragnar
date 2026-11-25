// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — LIERA's LightDM greeter
//
// Fullscreen GTK4 window running as LightDM's greeter-session
// (X11). Layers: animated LIERA artwork (see scene.rs), a
// centred login panel (login.rs) driven through liblightdm
// (greeter.rs), plus a power row for shutdown/restart/suspend.
//
//   ragnar-greeter            — normal mode (under lightdm)
//   ragnar-greeter --preview  — run on the desktop without a
//                               daemon, for artwork/animation work

mod ffi;
mod greeter;
mod i18n;
mod login;
mod scene;
mod util;

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, Application, ApplicationWindow, Box, CssProvider, Orientation};

use greeter::Auth;
use login::{power_buttons, LoginPanel};
use scene::Scene;

/// Application id — matches the .desktop file basename.
const APP_ID: &str = "org.lieraos.ragnar.Greeter";

/// Greeter styling — translucent panel over the artwork so the
/// wordmark/ornament stay unobstructed, gold accents matching
/// the GRUB menu selection.
const CSS: &str = "
.login-panel {
    background: alpha(#0a0d16, 0.55);
    border-radius: 12px;
    border: 1px solid alpha(#c8a96a, 0.35);
}
.login-panel entry,
.login-panel dropdown > button {
    background: alpha(#0e1220, 0.85);
    color: #e8e6df;
    border: 1px solid alpha(#c8a96a, 0.45);
    border-radius: 6px;
    padding: 6px 10px;
    caret-color: #c8a96a;
}
.login-panel entry:focus,
.login-panel dropdown > button:focus {
    border-color: #c8a96a;
}
.login-panel popover contents {
    background: #0e1220;
    color: #e8e6df;
}
.login-button {
    background: linear-gradient(180deg, #d9ba7d, #a8873f);
    color: #12100a;
    font-weight: 600;
    border-radius: 6px;
    padding: 7px 0;
}
.login-button:hover { background: linear-gradient(180deg, #e7cb92, #b69448); }
.login-message { color: #b9bfc9; font-size: 0.9em; }
.login-message.error { color: #e07070; }
.power-button {
    background: alpha(#0a0d16, 0.45);
    color: #d8d4c8;
    border-radius: 50%;
}
.power-button:hover { color: #c8a96a; background: alpha(#0a0d16, 0.75); }
/* Ambient sprites — gradients, not PNGs: a gold beam reads on the
   bright nebula where the white glow sprite never did. */
.glow-beam {
    background: radial-gradient(ellipse at center,
        alpha(#f0dcae, 0.9) 0%,
        alpha(#c8a96a, 0.4) 42%,
        transparent 74%);
}
.strike-flash {
    background: radial-gradient(ellipse at center,
        alpha(#ffffff, 0.95) 0%,
        alpha(#dfe8ff, 0.45) 45%,
        transparent 76%);
}
/* Clock overlay — dim gold, matches the panel palette. */
.greeter-clock {
    color: #d8cfae;
    text-shadow: 0 1px 8px alpha(#000000, 0.85);
}
/* Twinkling star specks — soft round points, no PNG needed. */
.twinkle {
    background: radial-gradient(circle,
        alpha(#ffffff, 0.95) 0%,
        alpha(#dfe8ff, 0.5) 40%,
        transparent 68%);
    border-radius: 50%;
}
";

fn main() {
    i18n::init();

    let preview = std::env::args().any(|a| a == "--preview");
    // GTK rejects unknown argv entries — feed it a filtered list.
    let raw: Vec<String> = std::env::args().filter(|a| a != "--preview").collect();
    let argv: Vec<&str> = raw.iter().map(String::as_str).collect();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, preview));
    app.run_with_args(&argv);
}

/// Compose the window: scene layers + login panel + power row.
fn build_ui(app: &Application, preview: bool) {
    load_css();

    // ---- daemon connection (absent in --preview) ---------------
    let greeter = if preview { None } else { ffi::Greeter::connect_daemon().ok() };
    let (hide_users, default_session) = greeter
        .as_ref()
        .map(|g| (g.hide_users(), g.default_session()))
        .unwrap_or_default();

    // ---- login panel --------------------------------------------
    let users = if preview {
        vec![("user".into(), "Live User".into())]
    } else {
        ffi::users()
    };
    let sessions = if preview {
        vec![("ragnar".into(), "Ragnar (Wayland)".into())]
    } else {
        ffi::sessions()
    };
    let panel = Rc::new(LoginPanel::new(
        &users,
        &sessions,
        &default_session,
        hide_users,
    ));

    // ---- backdrop + power row ------------------------------------
    // The scene owns the panel's position (centre-x, ~62 % down —
    // under the ornament band, like the GRUB menu).
    let scene_ref = Scene::new(&panel.widget);
    let scene_widget = scene_ref.borrow().widget();

    // Power row — real capabilities under the daemon, all enabled
    // in preview so the look can be checked (clicks only show a
    // message there; ffi::power without a daemon does nothing).
    let caps = if preview {
        (true, true, true, true)
    } else {
        ffi::can_power()
    };
    let row = Box::new(Orientation::Horizontal, 6);
    row.set_halign(gtk4::Align::End);
    row.set_valign(gtk4::Align::Start);
    row.set_margin_top(18);
    row.set_margin_end(18);
    let mut powered = false;
    for (button, action) in power_buttons(caps) {
        if preview {
            let panel = panel.clone();
            button.connect_clicked(move |_| {
                panel.set_message("Vorschau: Power-Aktion", false);
            });
        } else {
            button.connect_clicked(move |_| ffi::power(action));
        }
        row.append(&button);
        powered = true;
    }
    if powered {
        scene_widget.add_overlay(&row);
    }
    let _ = &greeter; // power uses the static liblightdm helpers

    // ---- clock overlay (top-left, like the panel clock the
    // gtk-greeter shows) -------------------------------------------
    let clock = gtk4::Label::new(None);
    clock.add_css_class("greeter-clock");
    clock.set_halign(gtk4::Align::Start);
    clock.set_valign(gtk4::Align::Start);
    clock.set_margin_top(14);
    clock.set_margin_start(18);
    let tick_clock = {
        let clock = clock.clone();
        move || {
            if let Ok(now) = gtk4::glib::DateTime::now_local() {
                let t = now.format("%H:%M").map(|s| s.to_string()).unwrap_or_default();
                let d = now
                    .format("%A, %-d. %B")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                clock.set_markup(&format!("<big>{t}</big>\n<small>{d}</small>"));
            }
            gtk4::glib::ControlFlow::Continue
        }
    };
    tick_clock.clone()();
    gtk4::glib::timeout_add_seconds_local(1, tick_clock);
    scene_widget.add_overlay(&clock);

    let window = ApplicationWindow::builder()
        .application(app)
        .decorated(!true) // frameless — lightdm owns the screen
        .child(&scene_widget)
        .build();

    if preview {
        // Fixed preview window — resizable windows get resized by
        // the WM, which feeds back into the cover scaling (zoom
        // loop) and churns the renderer.
        window.set_resizable(false);
        window.set_default_size(1280, 800);
        window.present();
    } else {
        // Greeter sessions have no window manager — fullscreen()
        // is a hint nobody honours, so we size the window to the
        // bounding box of ALL monitors ourselves: one scene spans
        // every screen, the login panel stays centred on the
        // primary monitor (the scene's anchor rect).
        if let Some(monitors) = gdk::Display::default().map(|d| d.monitors()) {
            let mut bbox = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for i in 0..monitors.n_items() {
                if let Some(m) = monitors.item(i).and_downcast::<gdk::Monitor>() {
                    let g = m.geometry();
                    bbox.0 = bbox.0.min(g.x());
                    bbox.1 = bbox.1.min(g.y());
                    bbox.2 = bbox.2.max(g.x() + g.width());
                    bbox.3 = bbox.3.max(g.y() + g.height());
                }
            }
            if bbox.2 > bbox.0 && bbox.3 > bbox.1 {
                window.set_default_size(bbox.2 - bbox.0, bbox.3 - bbox.1);
                if let Some(m0) = monitors.item(0).and_downcast::<gdk::Monitor>() {
                    let g = m0.geometry();
                    scene_ref.borrow_mut().set_anchor(
                        (g.x() - bbox.0) as f64,
                        (g.y() - bbox.1) as f64,
                        g.width() as f64,
                        g.height() as f64,
                    );
                }
            }
        }
        window.set_resizable(false);
        window.fullscreen();
        window.present();
        panel.user_entry.grab_focus();
    }

    // Wire the auth flow after the window exists; kick off a
    // timed autologin when the daemon asks for one.
    let auth = Auth::new(panel, greeter);
    auth.maybe_autologin();
}

/// Register the greeter stylesheet application-wide.
fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
