// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-livewall — animated live wallpaper for Ragnar
//
// One layer-shell surface per monitor on Layer::Background —
// the same slot swaybg occupies, so the component stays
// compositor-agnostic (works on labwc today, Odin later, per
// the neutral-interface rule).
//
//   ragnar-livewall              — stock aurora wallpaper
//   ragnar-livewall --image P    — animate a different picture
//
// Rendering pauses to zero cost while a fullscreen app covers
// the desktop (see watch.rs).

mod effects;
mod glwall;
mod shaders;
mod scene;
mod watch;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider, gdk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use scene::Scene;

/// Stock wallpaper — the same file the dconf preset points at.
const DEFAULT_IMAGE: &str = "/usr/share/backgrounds/lieraos/lieraos-default.png";

/// Lamp overlay styling — the aurora itself lives in the GLSL
/// shader on the wall widget, not in CSS sprites.
const CSS: &str = "
/* Warm spill on the facade around a lit window. */
.lamp-glow {
    background: radial-gradient(ellipse at center,
        alpha(#ffb347, 0.85) 0%,
        alpha(#ff8c1a, 0.35) 45%,
        transparent 75%);
}
/* Dark patch for blackout moments — the unlit window. */
.lamp-dark {
    background: alpha(#0a1420, 0.9);
}
";

fn main() {
    let image = image_arg();
    let app = Application::builder()
        .application_id("org.lieraos.ragnar.LiveWall")
        // NON_UNIQUE: ragnar-wallpaper spawns this as a child —
        // single-instance forwarding would make every spawn exit
        // immediately and wedge the daemon's respawn logic.
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| build_ui(app, &image));
    // Hand GTK only the program name — our own args (--image)
    // are already parsed, GTK must not see them.
    app.run_with_args(&["ragnar-livewall".to_string()]);
}

/// `--image PATH` or the stock wallpaper.
fn image_arg() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--image" {
            return args.get(i + 1).cloned().unwrap_or_default();
        }
    }
    DEFAULT_IMAGE.to_string()
}

/// Per monitor: a background-layer surface with the scene.
fn build_ui(app: &Application, image: &str) {
    load_css();

    // Shared pause flag — the watcher thread flips it when a
    // fullscreen toplevel exists on the seat.
    let paused = Arc::new(AtomicBool::new(false));
    watch::start(paused.clone());

    let Some(monitors) = gdk::Display::default().map(|d| d.monitors()) else {
        return;
    };
    for i in 0..monitors.n_items() {
        let Some(mon) = monitors.item(i).and_downcast::<gdk::Monitor>() else {
            continue;
        };
        let scene = Scene::new(image, paused.clone());
        let widget = scene.borrow().layer.clone();

        let window = ApplicationWindow::builder()
            .application(app)
            .child(&widget)
            .build();
        // Background layer, all edges anchored, no keyboard —
        // it behaves like a picture, not a window.
        window.init_layer_shell();
        window.set_layer(Layer::Background);
        window.set_namespace(Some("ragnar-livewall"));
        for e in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            window.set_anchor(e, true);
        }
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_monitor(Some(&mon));
        window.present();
    }
}

/// Register the veil stylesheet application-wide.
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
