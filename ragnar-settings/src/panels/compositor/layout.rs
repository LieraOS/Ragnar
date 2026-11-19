// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor panel: layout & behaviour
// Copyright (C) 2026 LieraOS Project
//
// The `<core>`, `<margins>`, `<desktops>`, `<focus>`, `<snapping>`
// and `<placement>` blocks: window gaps, work-area margins,
// workspace count, focus behaviour, snap distances, new-window
// placement and tearing.

use super::widgets::{attr_spin_row, choice_row, spin_row, switch_row};
use crate::i18n::msgid;
use crate::panels::{common, rcxml};
use gettextrs::gettext;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Focus-follows-mouse choices — const so the msgid markers promote.
const FOLLOW_OPTS: &[(&str, &str)] = &[("no", msgid("Off")), ("yes", msgid("On"))];

/// labwc placement policies.
const PLACEMENT_OPTS: &[(&str, &str)] = &[
    ("center", msgid("Center")),
    ("cursor", msgid("Under the cursor")),
    ("automatic", msgid("Automatic")),
    ("cascade", msgid("Cascade")),
];

/// The layout/behaviour cards.
pub fn section() -> gtk4::Box {
    let b = common::section(&gettext("Layout & behaviour"));

    // -- Gaps & work area --------------------------------------------
    let card = common::card();
    spin_row(
        &card,
        &gettext("Window gap"),
        &gettext("Space kept between tiled windows, in pixels."),
        "core",
        "gap",
        (0.0, 100.0),
        0.0,
    );
    attr_spin_row(
        &card,
        &gettext("Work-area margin: top"),
        &gettext("Pixels reserved at the top edge (e.g. for a dock)."),
        "margin",
        "top",
        (0.0, 200.0),
        0.0,
    );
    attr_spin_row(
        &card,
        &gettext("Work-area margin: bottom"),
        &gettext("Pixels reserved at the bottom edge."),
        "margin",
        "bottom",
        (0.0, 200.0),
        0.0,
    );
    attr_spin_row(
        &card,
        &gettext("Work-area margin: left"),
        &gettext("Pixels reserved at the left edge."),
        "margin",
        "left",
        (0.0, 200.0),
        0.0,
    );
    attr_spin_row(
        &card,
        &gettext("Work-area margin: right"),
        &gettext("Pixels reserved at the right edge."),
        "margin",
        "right",
        (0.0, 200.0),
        0.0,
    );
    b.append(&card);
    b.append(&common::note(&gettext(
        "Margins shrink the usable screen area — windows and panels \
         respect them when tiling or maximizing.",
    )));

    // -- Workspaces ----------------------------------------------------
    // The count spin is built manually (not via attr_spin_row) so a
    // change can also re-render the names card below.
    let card = common::card();
    let names_card = common::card();
    let entries: Rc<RefCell<Vec<gtk4::Entry>>> = Rc::new(RefCell::new(Vec::new()));
    let render_names = {
        let card = names_card.clone();
        let entries = entries.clone();
        Rc::new(move || {
            while let Some(c) = card.first_child() {
                card.remove(&c);
            }
            let n = rcxml::get_attr("desktops", "number")
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(4)
                .clamp(1, 12);
            let cur = rcxml::get_names();
            let mut store = entries.borrow_mut();
            store.clear();
            for i in 0..n {
                let e = gtk4::Entry::builder()
                    .text(cur.get(i).map(String::as_str).unwrap_or(""))
                    .placeholder_text(format!("{} {}", gettext("Workspace"), i + 1))
                    .valign(gtk4::Align::Center)
                    .build();
                {
                    // Commit on Enter or focus-leave — collect every
                    // entry, positional order is what labwc reads.
                    let all = entries.clone();
                    let save = move || {
                        let names: Vec<String> =
                            all.borrow().iter().map(|e| e.text().to_string()).collect();
                        rcxml::set_names(&names);
                    };
                    let s = save.clone();
                    e.connect_activate(move |_| s());
                    let focus = gtk4::EventControllerFocus::new();
                    focus.connect_leave(move |_| save());
                    e.add_controller(focus);
                }
                card.append(&common::row(
                    &format!("{} {}", gettext("Workspace"), i + 1),
                    "",
                    e.clone().upcast(),
                ));
                store.push(e);
            }
        })
    };
    render_names();
    {
        let cur = rcxml::get_attr("desktops", "number")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(4.0);
        let spin = common::spin(1.0, 12.0, 1.0);
        spin.set_value(cur);
        spin.set_valign(gtk4::Align::Center);
        let render = render_names.clone();
        spin.connect_value_changed(move |s| {
            rcxml::set_attr("desktops", "number", &(s.value() as i64).to_string());
            rcxml::reload();
            render();
        });
        card.append(&common::row(
            &gettext("Number of workspaces"),
            &gettext("How many virtual desktops exist."),
            spin.upcast(),
        ));
    }
    // popupTime is a child element of <desktops> (number is the
    // attribute — the docs' <desktops><popupTime> notation).
    spin_row(
        &card,
        &gettext("Workspace popup time"),
        &gettext("Milliseconds the workspace switch hint stays on screen."),
        "desktops",
        "popupTime",
        (0.0, 5000.0),
        1000.0,
    );
    b.append(&card);
    b.append(&names_card);
    b.append(&common::note(&gettext(
        "Names show in the workspace switcher. Empty fields keep \
         their generated name.",
    )));

    // -- Focus ----------------------------------------------------------
    let card = common::card();
    choice_row(
        &card,
        &gettext("Focus follows mouse"),
        &gettext("Focus the window under the pointer without clicking."),
        "focus",
        "followMouse",
        FOLLOW_OPTS,
        "no",
    );
    switch_row(
        &card,
        &gettext("Raise on focus"),
        &gettext("Bring the focused window to the front."),
        "focus",
        "raiseOnFocus",
        false,
    );
    spin_row(
        &card,
        &gettext("Raise delay"),
        &gettext("Milliseconds before a focused window is raised."),
        "focus",
        "raiseOnFocusDelay",
        (0.0, 3000.0),
        500.0,
    );
    b.append(&card);

    // -- Snapping & placement -------------------------------------------
    let card = common::card();
    attr_spin_row(
        &card,
        &gettext("Snap distance: windows"),
        &gettext("Pixels before a dragged window snaps to another window."),
        "range",
        "inner",
        (0.0, 50.0),
        10.0,
    );
    attr_spin_row(
        &card,
        &gettext("Snap distance: screen edges"),
        &gettext("Pixels before a dragged window snaps to a screen edge."),
        "range",
        "outer",
        (0.0, 50.0),
        10.0,
    );
    switch_row(
        &card,
        &gettext("Top edge maximizes"),
        &gettext("Dragging a window to the top edge maximizes it."),
        "snapping",
        "topMaximize",
        true,
    );
    choice_row(
        &card,
        &gettext("New window placement"),
        &gettext("Where new windows appear on screen."),
        "placement",
        "policy",
        PLACEMENT_OPTS,
        "center",
    );
    switch_row(
        &card,
        &gettext("Allow tearing"),
        &gettext("Fullscreen applications may bypass vsync for lower latency."),
        "core",
        "allowTearing",
        false,
    );
    b.append(&card);

    b
}
