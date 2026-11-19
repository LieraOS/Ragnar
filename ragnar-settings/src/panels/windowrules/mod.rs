// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Window rules" panel (advanced)
// Copyright (C) 2026 LieraOS Project
//
// Editor for the compositor's <windowRules>: which windows get which
// treatment when they open. Rows show the matcher and the effect
// summary; Add/Edit open the rule dialog; delete removes exactly
// that rule's span. Everything writes to rc.xml and reloads live.

mod dialog;
mod model;

use super::common;
use crate::i18n::msgid;
use crate::panels::rcxml;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation, Widget};
use std::rc::Rc;

/// Short human summary of a rule's effects for the list row.
fn summary(r: &model::Rule) -> String {
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, cond: bool, text: &str| {
        if cond {
            parts.push(gettext(text));
        }
    };
    push(
        &mut parts,
        r.has_action("ToggleAlwaysOnTop"),
        msgid("on top"),
    );
    push(
        &mut parts,
        r.has_action("ToggleAlwaysOnBottom"),
        msgid("below"),
    );
    push(
        &mut parts,
        r.has_action("ToggleOmnipresent"),
        msgid("all workspaces"),
    );
    push(&mut parts, r.has_action("Maximize"), msgid("maximized"));
    push(
        &mut parts,
        r.has_action("ToggleFullscreen"),
        msgid("fullscreen"),
    );
    push(&mut parts, r.has_action("Iconify"), msgid("minimized"));
    if let Some(d) = r.action_attr("SnapToEdge", "direction") {
        parts.push(format!("{} {d}", gettext("snap")));
    }
    if let Some(t) = r.action_attr("SendToDesktop", "to") {
        parts.push(format!("{} {t}", gettext("workspace")));
    }
    if r.action_attr("AutoPlace", "policy").as_deref() == Some("center") {
        parts.push(gettext("centered"));
    }
    match r.attr("serverDecoration") {
        Some("yes") => parts.push(gettext("compositor titlebar")),
        Some("no") => parts.push(gettext("no titlebar")),
        _ => {}
    }
    push(
        &mut parts,
        r.attr("skipTaskbar") == Some("yes"),
        msgid("no taskbar"),
    );
    push(
        &mut parts,
        r.attr("skipWindowSwitcher") == Some("yes"),
        msgid("no switcher"),
    );
    push(
        &mut parts,
        r.attr("fixedPosition") == Some("yes"),
        msgid("locked position"),
    );
    if parts.is_empty() {
        // A rule that only matches — still show what it carries.
        parts.push(gettext("custom actions"));
    }
    parts.join(" · ")
}

/// Title of the list row: the matcher description.
fn matcher(r: &model::Rule) -> String {
    match (r.attr("identifier"), r.attr("title")) {
        (Some(id), Some(t)) if !id.is_empty() && !t.is_empty() => {
            format!("{id} · “{t}”")
        }
        (Some(id), _) if !id.is_empty() => id.to_string(),
        (_, Some(t)) if !t.is_empty() => format!("“{t}”"),
        (Some(t), _) if !t.is_empty() => t.to_string(),
        _ => gettext("Every window"),
    }
}

/// Write `text` to rc.xml and reload the compositor.
fn apply(text: String, refresh: &Rc<dyn Fn()>) {
    let _ = std::fs::write(rcxml::rc_path(), text);
    rcxml::reload();
    refresh();
}

/// Fill `card` with one row per parsed rule.
fn render(card: &gtk4::ListBox, refresh: Rc<dyn Fn()>) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    let text = rcxml::read();
    let rules = model::parse(&text);
    if rules.is_empty() {
        card.append(&common::note(&gettext(
            "No rules yet — matching windows get compositor defaults.",
        )));
        return;
    }
    for (i, (r, _, _)) in rules.iter().enumerate() {
        let row_box = Box::new(Orientation::Horizontal, 6);
        row_box.set_valign(gtk4::Align::Center);
        let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        edit.set_tooltip_text(Some(&gettext("Edit rule")));
        del.set_tooltip_text(Some(&gettext("Delete rule")));
        row_box.append(&edit);
        row_box.append(&del);
        card.append(&common::row(&matcher(r), &summary(r), row_box.upcast()));

        {
            let refresh = refresh.clone();
            edit.connect_clicked(move |b| {
                let text = rcxml::read();
                let rules = model::parse(&text);
                if let Some((r, _, _)) = rules.get(i) {
                    let r = r.clone();
                    let refresh = refresh.clone();
                    dialog::open(
                        b,
                        Some(&r),
                        Rc::new(move |nr| {
                            apply(model::save_edit(&rcxml::read(), i, &nr), &refresh);
                        }),
                    );
                }
            });
        }
        {
            let refresh = refresh.clone();
            del.connect_clicked(move |_| {
                apply(model::save_delete(&rcxml::read(), i), &refresh);
            });
        }
    }
}

/// Build the window-rules panel widget.
pub fn build() -> Widget {
    let page = common::page(&gettext("Window rules"));
    page.append(&common::note(&gettext(
        "Rules give matching windows their own placement, state and \
         decoration when they open. Changes apply immediately — no \
         restart needed.",
    )));

    let card = common::card();
    // The refresh closure re-renders the card and hands each row the
    // same callback — the RefCell breaks the self-reference cycle.
    let cell = std::rc::Rc::new(std::cell::RefCell::new(None::<Rc<dyn Fn()>>));
    let refresh: Rc<dyn Fn()> = {
        let card = card.clone();
        let cell2 = cell.clone();
        let f = Rc::new(move || {
            let r = cell2.borrow().as_ref().unwrap().clone();
            render(&card, r);
        });
        *cell.borrow_mut() = Some(f.clone());
        f
    };
    render(&card, refresh.clone());
    page.append(&card);

    let add = gtk4::Button::with_label(&gettext("Add rule"));
    add.set_halign(gtk4::Align::Start);
    add.add_css_class("suggested-action");
    {
        let refresh = refresh.clone();
        add.connect_clicked(move |b| {
            let refresh = refresh.clone();
            dialog::open(
                b,
                None,
                Rc::new(move |r| {
                    apply(model::save_add(&rcxml::read(), &r), &refresh);
                }),
            );
        });
    }
    page.append(&add);

    page.upcast()
}
