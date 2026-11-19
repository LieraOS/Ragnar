// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — window-rule editor dialog
// Copyright (C) 2026 LieraOS Project
//
// Modal editor for one rule: matchers on top, then the structured
// behaviours — window state, placement, appearance. Everything the
// model does not cover (extra attributes, unknown actions, comments)
// survives untouched via the verbatim inner text.

use super::model::Rule;
use crate::i18n::msgid;
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::rc::Rc;

/// labwc SnapToEdge directions — the empty id means "not set".
const SNAPS: &[(&str, &str)] = &[
    ("", msgid("Don't snap")),
    ("left", msgid("Left half")),
    ("right", msgid("Right half")),
    ("up", msgid("Top half")),
    ("down", msgid("Bottom half")),
    ("up-left", msgid("Top left quarter")),
    ("up-right", msgid("Top right quarter")),
    ("down-left", msgid("Bottom left quarter")),
    ("down-right", msgid("Bottom right quarter")),
    ("center", msgid("Center")),
];

/// serverDecoration property states.
const DECOS: &[(&str, &str)] = &[
    ("", msgid("Default")),
    ("yes", msgid("Compositor-drawn")),
    ("no", msgid("None")),
];

/// Collect the dialog's state into a rule, starting from `base`
/// so unmodelled content survives.
fn collect(base: &Rule, ctls: &Ctls) -> Rule {
    let mut r = base.clone();
    r.set_attr(
        "identifier",
        Some(ctls.ident.text().trim()).filter(|s| !s.is_empty()),
    );
    r.set_attr(
        "title",
        Some(ctls.title.text().trim()).filter(|s| !s.is_empty()),
    );
    for (key, cb) in &ctls.prop_bools {
        r.set_attr(key, cb.is_active().then_some("yes"));
    }
    r.set_attr(
        "serverDecoration",
        DECOS
            .get(ctls.deco.selected() as usize)
            .map(|(id, _)| *id)
            .filter(|s| !s.is_empty()),
    );
    for (name, cb) in &ctls.act_bools {
        if cb.is_active() {
            if !r.has_action(name) {
                r.add_action(&format!("<action name=\"{name}\" />"));
            }
        } else {
            r.remove_action(name);
        }
    }
    if let Some((dir, _)) = SNAPS.get(ctls.snap.selected() as usize) {
        r.remove_action("SnapToEdge");
        if !dir.is_empty() {
            r.add_action(&format!(
                "<action name=\"SnapToEdge\" direction=\"{dir}\" />"
            ));
        }
    }
    r.remove_action("SendToDesktop");
    let ws = ctls.workspace.value() as i64;
    if ws > 0 {
        r.add_action(&format!(
            "<action name=\"SendToDesktop\" to=\"{ws}\" follow=\"no\" />"
        ));
    }
    r
}

/// Widget handles the collect step needs.
struct Ctls {
    ident: gtk4::Entry,
    title: gtk4::Entry,
    prop_bools: Vec<(&'static str, gtk4::CheckButton)>,
    act_bools: Vec<(&'static str, gtk4::CheckButton)>,
    deco: gtk4::DropDown,
    snap: gtk4::DropDown,
    workspace: gtk4::SpinButton,
}

fn entry_row(card: &gtk4::ListBox, title: &str, subtitle: &str, text: &str) -> gtk4::Entry {
    let e = gtk4::Entry::builder()
        .text(text)
        .valign(gtk4::Align::Center)
        .build();
    card.append(&common::row(title, subtitle, e.clone().upcast()));
    e
}

fn check_row(card: &gtk4::ListBox, title: &str, subtitle: &str, active: bool) -> gtk4::CheckButton {
    let cb = gtk4::CheckButton::builder()
        .active(active)
        .valign(gtk4::Align::Center)
        .build();
    card.append(&common::row(title, subtitle, cb.clone().upcast()));
    cb
}

fn drop_row(
    card: &gtk4::ListBox,
    title: &str,
    subtitle: &str,
    opts: &'static [(&'static str, &'static str)],
    cur_id: &str,
) -> gtk4::DropDown {
    let sel = opts.iter().position(|(id, _)| *id == cur_id).unwrap_or(0) as u32;
    let labels: Vec<String> = opts.iter().map(|(_, l)| gettext(*l)).collect();
    let dd = gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    dd.set_selected(sel);
    dd.set_valign(gtk4::Align::Center);
    card.append(&common::row(title, subtitle, dd.clone().upcast()));
    dd
}

/// Open the editor for `rule` (None = new rule). `on_save` receives
/// the finished rule.
pub fn open(anchor: &impl IsA<gtk4::Widget>, rule: Option<&Rule>, on_save: Rc<dyn Fn(Rule)>) {
    let base = rule.cloned().unwrap_or(Rule {
        attrs: vec![
            ("identifier".into(), String::new()),
            ("title".into(), String::new()),
        ],
        inner: String::new(),
    });
    let editing = rule.is_some();

    let win = gtk4::Window::builder()
        .title(gettext(if editing {
            msgid("Edit rule")
        } else {
            msgid("New window rule")
        }))
        .modal(true)
        .default_width(460)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 14);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_min_content_height(300);
    scroll.set_max_content_height(440);
    scroll.set_propagate_natural_height(true);
    let content = Box::new(Orientation::Vertical, 14);

    // -- Match -----------------------------------------------------
    let s = common::section(&gettext("Match"));
    let card = common::card();
    let ident = entry_row(
        &card,
        &gettext("App identifier"),
        &gettext("Application id, e.g. firefox or steam_app_*. Glob: * any text, ? one character."),
        base.attr("identifier").unwrap_or(""),
    );
    let title = entry_row(
        &card,
        &gettext("Window title"),
        &gettext("Title pattern (optional). All given criteria must match."),
        base.attr("title").unwrap_or(""),
    );
    let match_once = check_row(
        &card,
        &gettext("Only the first matching window"),
        &gettext("Apply the rule once instead of to every matching window."),
        base.attr("matchOnce") == Some("true"),
    );
    s.append(&card);
    content.append(&s);

    // -- Window state ----------------------------------------------
    let s = common::section(&gettext("Window state"));
    let card = common::card();
    let mut act_bools: Vec<(&'static str, gtk4::CheckButton)> = Vec::new();
    for (name, t, sub) in [
        (
            "ToggleAlwaysOnTop",
            msgid("Always on top"),
            msgid("Keep the window above all others."),
        ),
        (
            "ToggleAlwaysOnBottom",
            msgid("Keep below"),
            msgid("Keep the window below all others."),
        ),
        (
            "ToggleOmnipresent",
            msgid("On all workspaces"),
            msgid("Show the window on every workspace."),
        ),
        (
            "Maximize",
            msgid("Start maximized"),
            msgid("Open the window maximized."),
        ),
        (
            "ToggleFullscreen",
            msgid("Start fullscreen"),
            msgid("Open the window fullscreen."),
        ),
        (
            "Iconify",
            msgid("Start minimized"),
            msgid("Open the window minimized."),
        ),
    ] {
        let cb = check_row(&card, &gettext(t), &gettext(sub), base.has_action(name));
        act_bools.push((name, cb));
    }
    s.append(&card);
    content.append(&s);

    // -- Placement ---------------------------------------------------
    let s = common::section(&gettext("Placement"));
    let card = common::card();
    let snap_cur = base
        .action_attr("SnapToEdge", "direction")
        .unwrap_or_default();
    let snap = drop_row(
        &card,
        &gettext("Snap to"),
        &gettext("Tile the window when it opens."),
        SNAPS,
        &snap_cur,
    );
    // AutoPlace lives outside act_bools — it carries a policy arg
    // and is applied in the save handler like SnapToEdge.
    let center = check_row(
        &card,
        &gettext("Center on screen"),
        &gettext("Place the window in the middle when it opens."),
        base.action_attr("AutoPlace", "policy").as_deref() == Some("center"),
    );
    let ws_cur = base
        .action_attr("SendToDesktop", "to")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let workspace = common::spin(0.0, 12.0, 1.0);
    workspace.set_value(ws_cur);
    workspace.set_valign(gtk4::Align::Center);
    card.append(&common::row(
        &gettext("Open on workspace"),
        &gettext("Workspace number to open the window on (0 = current)."),
        workspace.clone().upcast(),
    ));
    s.append(&card);
    content.append(&s);

    // -- Appearance & behaviour --------------------------------------
    let s = common::section(&gettext("Appearance & behaviour"));
    let card = common::card();
    let deco = drop_row(
        &card,
        &gettext("Titlebar"),
        &gettext("Override window decoration for matching windows."),
        DECOS,
        base.attr("serverDecoration").unwrap_or(""),
    );
    let mut prop_bools: Vec<(&'static str, gtk4::CheckButton)> = Vec::new();
    for (key, t, sub) in [
        (
            "skipTaskbar",
            msgid("Hide from taskbar"),
            msgid("The window does not appear in panels and docks."),
        ),
        (
            "skipWindowSwitcher",
            msgid("Hide from window switcher"),
            msgid("The window is skipped by the switcher."),
        ),
        (
            "ignoreFocusRequest",
            msgid("Block self-focus"),
            msgid("The window cannot activate itself."),
        ),
        (
            "fixedPosition",
            msgid("Lock position"),
            msgid("The window cannot be moved or resized interactively."),
        ),
    ] {
        let cb = check_row(
            &card,
            &gettext(t),
            &gettext(sub),
            base.attr(key) == Some("yes"),
        );
        prop_bools.push((key, cb));
    }
    s.append(&card);
    content.append(&s);

    scroll.set_child(Some(&content));
    body.append(&scroll);

    // -- Footer ------------------------------------------------------
    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let apply = gtk4::Button::with_label(&gettext(if editing {
        msgid("Apply")
    } else {
        msgid("Add rule")
    }));
    apply.add_css_class("suggested-action");
    apply.set_sensitive(false);
    btns.append(&cancel);
    btns.append(&apply);
    body.append(&btns);

    // A rule needs at least one criterion — otherwise it would match
    // every window.
    {
        let (a, i, t) = (apply.clone(), ident.clone(), title.clone());
        let check = move || {
            a.set_sensitive(!i.text().trim().is_empty() || !t.text().trim().is_empty());
        };
        let c = check.clone();
        ident.connect_changed(move |_| c());
        title.connect_changed(move |_| check());
        // Pre-fill state for edits.
        apply.set_sensitive(
            !base.attr("identifier").unwrap_or("").is_empty()
                || !base.attr("title").unwrap_or("").is_empty(),
        );
    }

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    let w = win.clone();
    apply.connect_clicked(move |_| {
        let ctls = Ctls {
            ident: ident.clone(),
            title: title.clone(),
            prop_bools: prop_bools.iter().map(|(k, c)| (*k, c.clone())).collect(),
            act_bools: act_bools.iter().map(|(k, c)| (*k, c.clone())).collect(),
            deco: deco.clone(),
            snap: snap.clone(),
            workspace: workspace.clone(),
        };
        // Center-on-open is its own action toggle.
        let mut r = collect(&base, &ctls);
        r.remove_action("AutoPlace");
        if center.is_active() {
            r.add_action("<action name=\"AutoPlace\" policy=\"center\" />");
        }
        if match_once.is_active() {
            r.set_attr("matchOnce", Some("true"));
        } else {
            r.set_attr("matchOnce", None);
        }
        on_save(r);
        w.close();
    });

    win.set_child(Some(&body));
    win.present();
}
