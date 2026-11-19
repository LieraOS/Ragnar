// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Compositor" panel (advanced)
// Copyright (C) 2026 LieraOS Project
//
// Deep window-manager configuration written straight into the
// compositor's rc.xml — the file is the interface, exactly the
// knobs that would otherwise mean hand-editing configuration.
// Every section declares its capabilities: rows the running
// compositor cannot do are shown locked, and unlock themselves
// once the native Ragnar compositor runs.

mod decoration;
mod future;
mod layout;
mod widgets;

use super::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::Widget;
use libadwaita as adw;

/// Build the compositor panel widget.
pub fn build() -> Widget {
    let page = common::page(&gettext("Compositor"));

    // Marked experimental on purpose: several controls only reach
    // compositor-decorated windows, and not every option is verified
    // on every application type yet.
    let banner = adw::Banner::new(&gettext(
        "Experimental — not every option is confirmed on all \
         applications yet",
    ));
    banner.add_css_class("warning");
    banner.set_revealed(true);
    page.append(&banner);

    page.append(&common::note(&gettext(
        "Every change applies immediately — no restart or re-login \
         needed. Controls the running compositor cannot provide are \
         shown locked.",
    )));

    page.append(&decoration::section());
    page.append(&layout::section());
    page.append(&future::section());

    page.upcast()
}
