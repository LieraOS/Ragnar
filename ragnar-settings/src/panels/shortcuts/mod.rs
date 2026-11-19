// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Keyboard shortcuts" panel
// Copyright (C) 2026 LieraOS Project
//
// Lists every <keybind> in the compositor's rc.xml. Binds carrying
// managed="labwc-bridge" belong to the session bridge (it rebuilds
// them from GSettings) and are shown read-only; every other bind is
// editable and deletable. New binds support the full WM-action
// catalogue, not just running commands. All writes reload live.

mod dialog;
mod model;

use crate::i18n::msgid;
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::rc::Rc;

/// German locale → "Strg", "Umschalt" etc.; English fallback.
fn mod_names() -> (&'static str, &'static str, &'static str, &'static str) {
    if std::env::var("LANG")
        .map(|l| l.starts_with("de"))
        .unwrap_or(false)
    {
        ("Strg", "Alt", "Umschalt", "Super")
    } else {
        ("Ctrl", "Alt", "Shift", "Super")
    }
}

/// "C-S-Escape" → "Strg+Umschalt+Esc" — labwc notation out.
pub fn human_key(key: &str) -> String {
    let (c, a, s, w) = mod_names();
    let mut out = Vec::new();
    let mut rest = key;
    while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
        out.push(match &rest[..1] {
            "C" => c,
            "A" => a,
            "S" => s,
            "W" => w,
            _ => &rest[..1],
        });
        rest = &rest[2..];
    }
    out.push(rest);
    out.join("+")
}

/// Action → readable description, including args where useful.
fn describe(b: &model::Bind) -> String {
    let a = &b.action;
    if b.extra_actions {
        return format!("{} (+{})", a.name, gettext("more actions"));
    }
    match a.name.as_str() {
        "Execute" => describe_cmd(a.attr("command").unwrap_or("")),
        "Close" => gettext("Close window"),
        "Kill" => gettext("Force-close window"),
        "Iconify" => gettext("Minimize window"),
        "ToggleMaximize" => gettext("Toggle maximize"),
        "ToggleFullscreen" => gettext("Toggle fullscreen"),
        "SnapToEdge" | "ToggleSnapToEdge" => {
            format!(
                "{} {}",
                gettext("Snap to"),
                a.attr("direction").unwrap_or("")
            )
        }
        "GoToDesktop" => format!(
            "{} {}",
            gettext("Go to workspace"),
            a.attr("to").unwrap_or("")
        ),
        "SendToDesktop" => format!(
            "{} {}",
            gettext("Move window to workspace"),
            a.attr("to").unwrap_or("")
        ),
        "NextWindow" => gettext("Next window"),
        "PreviousWindow" => gettext("Previous window"),
        "ShowMenu" => gettext("Show desktop menu"),
        "ToggleShowDesktop" => gettext("Show/hide desktop"),
        "Reconfigure" => gettext("Reload compositor configuration"),
        "" => gettext("(no action)"),
        other => other.to_string(),
    }
}

/// Raw command → readable description. Strips the dbus-send
/// plumbing and names the action instead.
fn describe_cmd(cmd: &str) -> String {
    let known: &[(&str, &str)] = &[
        ("MprisController.PlayPause", msgid("Media: Play/Pause")),
        ("MprisController.Play", msgid("Media: Play")),
        ("MprisController.Next", msgid("Media: Next track")),
        ("MprisController.Previous", msgid("Media: Previous track")),
        ("MprisController.Stop", msgid("Media: Stop")),
        ("ShowMenu", msgid("Show application menu")),
        ("org.lieraos.ragnar.Screenshot", msgid("Take a screenshot")),
        ("org.lieraos.ragnar.PowerDialog", msgid("Show power dialog")),
        ("org.lieraos.ragnar.Lockscreen", msgid("Lock the screen")),
        ("org.lieraos.ragnar.Screenlock", msgid("Lock the screen")),
    ];
    for (pat, label) in known {
        if cmd.contains(pat) {
            return gettext(*label);
        }
    }
    cmd.split_whitespace()
        .rfind(|t| !t.starts_with('-'))
        .unwrap_or(cmd)
        .to_string()
}

/// Display row: key combo chip + description + buttons.
fn bind_row(b: &model::Bind, extra: gtk4::Widget) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 14);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 10);
    }
    let chip = gtk4::Frame::new(None);
    chip.add_css_class("view");
    let k = gtk4::Label::new(Some(&human_key(&b.key)));
    for m in ["start", "end"] {
        k.set_property(&format!("margin-{m}"), 8);
    }
    for m in ["top", "bottom"] {
        k.set_property(&format!("margin-{m}"), 3);
    }
    k.set_valign(gtk4::Align::Center);
    chip.set_child(Some(&k));
    chip.set_valign(gtk4::Align::Center);
    inner.append(&chip);

    let c = gtk4::Label::new(Some(&describe(b)));
    c.set_xalign(0.0);
    c.set_hexpand(true);
    c.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    c.set_valign(gtk4::Align::Center);
    inner.append(&c);
    inner.append(&extra);
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Write `text` back to rc.xml and reload the compositor.
fn apply(text: String, refresh: &Rc<dyn Fn()>) {
    let _ = std::fs::write(crate::panels::rcxml::rc_path(), text);
    crate::panels::rcxml::reload();
    refresh();
}

/// Fill `card` with rows for all binds; `managed` selects which set.
fn render(card: &ListBox, managed: bool, refresh: Rc<dyn Fn()>) {
    while let Some(c) = card.first_child() {
        card.remove(&c);
    }
    let text = std::fs::read_to_string(crate::panels::rcxml::rc_path()).unwrap_or_default();
    let binds = model::parse(&text);
    for (i, (b, _, _)) in binds.iter().enumerate() {
        if b.managed != managed {
            continue;
        }
        if managed {
            let lock = gtk4::Image::from_icon_name("system-lock-screen-symbolic");
            lock.set_tooltip_text(Some(&gettext(
                "Managed by the desktop session — configure via keybinding settings",
            )));
            card.append(&bind_row(b, lock.upcast()));
            continue;
        }
        let row_box = Box::new(Orientation::Horizontal, 6);
        row_box.set_valign(gtk4::Align::Center);
        let edit = gtk4::Button::from_icon_name("document-edit-symbolic");
        let del = gtk4::Button::from_icon_name("user-trash-symbolic");
        edit.set_tooltip_text(Some(&gettext("Edit shortcut")));
        del.set_tooltip_text(Some(&gettext("Delete shortcut")));
        row_box.append(&edit);
        row_box.append(&del);
        card.append(&bind_row(b, row_box.upcast()));
        {
            let refresh = refresh.clone();
            let b = b.clone();
            edit.connect_clicked(move |btn| {
                let refresh = refresh.clone();
                let b = b.clone();
                dialog::open(
                    btn,
                    Some(&b),
                    Rc::new(move |key, act| {
                        apply(
                            model::save_edit(
                                &std::fs::read_to_string(crate::panels::rcxml::rc_path())
                                    .unwrap_or_default(),
                                i,
                                &key,
                                &act,
                            ),
                            &refresh,
                        );
                    }),
                );
            });
        }
        {
            let refresh = refresh.clone();
            del.connect_clicked(move |_| {
                apply(
                    model::save_delete(
                        &std::fs::read_to_string(crate::panels::rcxml::rc_path())
                            .unwrap_or_default(),
                        i,
                    ),
                    &refresh,
                );
            });
        }
    }
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = common::page(&gettext("Keyboard shortcuts"));

    // Session-managed binds: read-only, the bridge owns them.
    let s = common::section(&gettext("Session shortcuts"));
    s.append(&common::note(&gettext(
        "Provided and managed by the desktop session — shown here \
         for reference.",
    )));
    let managed_card = common::card();
    render(&managed_card, true, Rc::new(|| {}));
    s.append(&managed_card);
    page.append(&s);

    // User-owned binds: fully editable.
    let s = common::section(&gettext("Shortcuts"));
    let card = common::card();
    let cell = std::rc::Rc::new(std::cell::RefCell::new(None::<Rc<dyn Fn()>>));
    let refresh: Rc<dyn Fn()> = {
        let card = card.clone();
        let cell2 = cell.clone();
        let f = Rc::new(move || {
            let r = cell2.borrow().as_ref().unwrap().clone();
            render(&card, false, r);
        });
        *cell.borrow_mut() = Some(f.clone());
        f
    };
    render(&card, false, refresh.clone());
    s.append(&card);
    s.append(&common::note(&gettext(
        "Changes apply immediately — press the shortcut right away \
         to try it.",
    )));
    page.append(&s);

    let add = gtk4::Button::with_label(&gettext("Add shortcut…"));
    add.set_halign(gtk4::Align::Start);
    add.add_css_class("suggested-action");
    {
        let refresh = refresh.clone();
        add.connect_clicked(move |b| {
            let refresh = refresh.clone();
            dialog::open(
                b,
                None,
                Rc::new(move |key, act| {
                    apply(
                        model::save_add(
                            &std::fs::read_to_string(crate::panels::rcxml::rc_path())
                                .unwrap_or_default(),
                            &key,
                            &act,
                        ),
                        &refresh,
                    );
                }),
            );
        });
    }
    page.append(&add);

    page.upcast()
}
