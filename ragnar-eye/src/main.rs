// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — fast image viewer for the Ragnar desktop.
//
// GTK4 image viewer with a classic three-pane layout, async
// decoding, free zoom and 50+ formats via the image crate,
// runtime-loaded system codec libraries and hand-rolled
// decoders for exotic formats.

use gtk4::prelude::*;
use std::path::PathBuf;

fn main() {
    // GTK4's GPU renderers compile shaders on first frame — that
    // costs ~2 s of dead window time on cold start, and Vulkan on
    // nvidia is flaky on top. The cairo renderer draws instantly
    // and is plenty fast for a viewer (the canvas blits through
    // cairo anyway). Users can still override via GSK_RENDERER.
    if std::env::var_os("GSK_RENDERER").is_none() {
        unsafe { std::env::set_var("GSK_RENDERER", "cairo") };
    }
    ragnar_eye::i18n::init();
    let app = gtk4::Application::builder()
        .application_id("org.lieraos.ragnar.Eye")
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    // File-manager open (%F): show the first file immediately.
    app.connect_open(|app, files, _| {
        let p = files.first().and_then(|f| f.path()).map(PathBuf::from);
        ragnar_eye::window::run(app, p);
    });
    // Plain launch: open in the current directory.
    app.connect_activate(|app| ragnar_eye::window::run(app, None));

    app.run();
}
