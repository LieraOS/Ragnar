// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — keybind editor dialog
// Copyright (C) 2026 LieraOS Project
//
// Modal editor for one keybind: a grab button captures the combo in
// labwc notation, an action dropdown picks what the key does and an
// argument row adapts to the action (command, direction, workspace,
// menu name). Editing a bind whose action is not in the catalogue
// keeps that action verbatim — only the key is re-bound.

use super::human_key;
use super::model::{Action, Bind};
use crate::i18n::msgid;
use crate::panels::common;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

/// What extra input an action needs.
#[derive(Clone, Copy, PartialEq)]
enum Arg {
    None,
    Command,
    Direction,
    Desktop,
    Menu,
}

/// The action catalogue shown in the dropdown — labwc actions a
// desktop user plausibly binds to keys.
const ACTIONS: &[(&str, &str, Arg)] = &[
    ("Execute", msgid("Run a command"), Arg::Command),
    ("Close", msgid("Close window"), Arg::None),
    ("Kill", msgid("Force-close window"), Arg::None),
    ("Iconify", msgid("Minimize window"), Arg::None),
    ("ToggleMaximize", msgid("Toggle maximize"), Arg::None),
    ("ToggleFullscreen", msgid("Toggle fullscreen"), Arg::None),
    ("SnapToEdge", msgid("Snap window to edge"), Arg::Direction),
    ("GoToDesktop", msgid("Go to workspace"), Arg::Desktop),
    (
        "SendToDesktop",
        msgid("Move window to workspace"),
        Arg::Desktop,
    ),
    ("NextWindow", msgid("Next window"), Arg::None),
    ("PreviousWindow", msgid("Previous window"), Arg::None),
    ("ShowMenu", msgid("Show desktop menu"), Arg::Menu),
    ("ToggleShowDesktop", msgid("Show/hide desktop"), Arg::None),
    (
        "Reconfigure",
        msgid("Reload compositor configuration"),
        Arg::None,
    ),
];

/// SnapToEdge directions — empty id means "not set" (not offered here).
const DIRS: &[(&str, &str)] = &[
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

/// Values read back from the dynamic argument row.
#[derive(Default)]
struct ArgVals {
    dir_idx: usize,
    cmd: String,
    desktop: i64,
    follow: bool,
    menu: String,
}

/// Reader closure installed by the current argument row.
type ArgReader = Rc<dyn Fn() -> ArgVals>;

/// Keys that are modifiers themselves — never captured as the key.
fn is_modifier(name: &str) -> bool {
    matches!(
        name,
        "Shift_L"
            | "Shift_R"
            | "Control_L"
            | "Control_R"
            | "Alt_L"
            | "Alt_R"
            | "Super_L"
            | "Super_R"
            | "Meta_L"
            | "Meta_R"
            | "Hyper_L"
            | "Hyper_R"
            | "ISO_Level3_Shift"
    )
}

/// Build the labwc combo string from a captured key press.
fn combo(kv: gtk4::gdk::Key, state: gtk4::gdk::ModifierType) -> Option<String> {
    let name = kv.name()?;
    if is_modifier(&name) {
        return None;
    }
    let mut mods = String::new();
    if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
        mods.push_str("C-");
    }
    if state.contains(gtk4::gdk::ModifierType::ALT_MASK) {
        mods.push_str("A-");
    }
    if state.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
        mods.push_str("S-");
    }
    if state.contains(gtk4::gdk::ModifierType::SUPER_MASK) {
        mods.push_str("W-");
    }
    Some(format!("{mods}{name}"))
}

/// The arg control row shown under the action dropdown.
fn arg_row(arg: Arg, bind: Option<&Bind>) -> (gtk4::Widget, ArgReader) {
    let row = Box::new(Orientation::Horizontal, 8);
    let cmd_def = bind
        .and_then(|b| b.action.attr("command"))
        .unwrap_or("")
        .to_string();
    let dir_def = bind
        .and_then(|b| b.action.attr("direction"))
        .unwrap_or("left")
        .to_string();
    let to_def = bind
        .and_then(|b| b.action.attr("to"))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    let follow_def = bind.and_then(|b| b.action.attr("follow")) != Some("no");
    let menu_def = bind
        .and_then(|b| b.action.attr("menu"))
        .unwrap_or("root-menu")
        .to_string();

    match arg {
        Arg::None => {
            let read: ArgReader = Rc::new(ArgVals::default);
            (row.upcast(), read)
        }
        Arg::Command => {
            let e = gtk4::Entry::builder()
                .text(&cmd_def)
                .placeholder_text(gettext("Command to run, e.g. firefox"))
                .hexpand(true)
                .build();
            row.append(&e);
            let read: ArgReader = Rc::new(move || ArgVals {
                cmd: e.text().to_string(),
                ..Default::default()
            });
            (row.upcast(), read)
        }
        Arg::Direction => {
            let labels: Vec<String> = DIRS.iter().map(|(_, l)| gettext(*l)).collect();
            let dd = gtk4::DropDown::from_strings(
                &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            let sel = DIRS.iter().position(|(id, _)| *id == dir_def).unwrap_or(0) as u32;
            dd.set_selected(sel);
            row.append(&dd);
            let read: ArgReader = Rc::new(move || ArgVals {
                dir_idx: dd.selected() as usize,
                ..Default::default()
            });
            (row.upcast(), read)
        }
        Arg::Desktop => {
            let s = common::spin(1.0, 12.0, 1.0);
            s.set_value(to_def);
            row.append(&s);
            let f = gtk4::CheckButton::with_label(&gettext("Switch to it as well"));
            f.set_active(follow_def);
            row.append(&f);
            let read: ArgReader = Rc::new(move || ArgVals {
                desktop: s.value() as i64,
                follow: f.is_active(),
                ..Default::default()
            });
            (row.upcast(), read)
        }
        Arg::Menu => {
            let e = gtk4::Entry::builder()
                .text(&menu_def)
                .placeholder_text(gettext("Menu name, e.g. root-menu"))
                .hexpand(true)
                .build();
            row.append(&e);
            let read: ArgReader = Rc::new(move || ArgVals {
                menu: e.text().to_string(),
                ..Default::default()
            });
            (row.upcast(), read)
        }
    }
}

/// Open the editor. `bind` = None for a new bind; `on_save` receives
/// the key and the action to write.
pub fn open(
    anchor: &impl IsA<gtk4::Widget>,
    bind: Option<&Bind>,
    on_save: Rc<dyn Fn(String, Action)>,
) {
    let editing = bind.is_some();
    let known_idx = bind.and_then(|b| ACTIONS.iter().position(|(n, _, _)| *n == b.action.name));

    let win = gtk4::Window::builder()
        .title(gettext(if editing {
            msgid("Edit shortcut")
        } else {
            msgid("Add shortcut")
        }))
        .modal(true)
        .default_width(430)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 12);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }

    // -- Key capture ----------------------------------------------------
    let grab = gtk4::Button::with_label(&gettext("Press shortcut…"));
    if let Some(b) = bind {
        grab.set_label(&human_key(&b.key));
    }
    body.append(&grab);

    // -- Action dropdown --------------------------------------------------
    let mut labels: Vec<String> = ACTIONS.iter().map(|(_, l, _)| gettext(*l)).collect();
    // Unknown action on edit: offered as a verbatim keep option.
    let keep_idx = if editing && known_idx.is_none() {
        labels.insert(
            0,
            format!("{}: {}", gettext("Keep action"), bind.unwrap().action.name),
        );
        Some(0)
    } else {
        None
    };
    let action_dd =
        gtk4::DropDown::from_strings(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    let sel = keep_idx.or(known_idx.map(|i| i + keep_idx.map_or(0, |_| 0)));
    action_dd.set_selected(sel.unwrap_or(0) as u32);
    body.append(&common::row(
        &gettext("Action"),
        "",
        action_dd.clone().upcast(),
    ));

    // -- Dynamic argument row ----------------------------------------------
    let arg_host = Box::new(Orientation::Vertical, 0);
    body.append(&arg_host);
    let reader: Rc<RefCell<Option<ArgReader>>> = Rc::new(RefCell::new(None));
    let set_arg = {
        let host = arg_host.clone();
        let reader = reader.clone();
        let bind = bind.cloned();
        move |idx: usize| {
            while let Some(c) = host.first_child() {
                host.remove(&c);
            }
            // Map dropdown index → ACTION entry (skipping keep slot).
            let real = match keep_idx {
                Some(_) if idx == 0 => return, // keep-action: no args
                Some(_) => idx - 1,
                None => idx,
            };
            let (_, _, arg) = ACTIONS[real];
            let (w, r) = arg_row(arg, bind.as_ref());
            host.append(&w);
            *reader.borrow_mut() = Some(r);
        }
    };
    set_arg(action_dd.selected() as usize);
    {
        let set_arg = set_arg.clone();
        action_dd.connect_selected_notify(move |dd| set_arg(dd.selected() as usize));
    }

    // -- Footer -------------------------------------------------------------
    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let ok = gtk4::Button::with_label(&gettext(if editing {
        msgid("Apply")
    } else {
        msgid("Add")
    }));
    ok.add_css_class("suggested-action");
    btns.append(&cancel);
    btns.append(&ok);
    body.append(&btns);

    // Key capture controller — fixed: every modifier carries its dash,
    // pure modifier presses are ignored.
    let captured = Rc::new(RefCell::new(
        bind.map(|b| b.key.clone()).unwrap_or_default(),
    ));
    let keyc = gtk4::EventControllerKey::new();
    keyc.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let (g, cap) = (grab.clone(), captured.clone());
        keyc.connect_key_pressed(move |_, kv, _, state| {
            if let Some(c) = combo(kv, state) {
                *cap.borrow_mut() = c.clone();
                g.set_label(&human_key(&c));
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
    }
    win.add_controller(keyc);
    grab.connect_clicked(|b| b.set_label(&gettext("Press shortcut…")));

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    {
        let (w, cap, orig) = (win.clone(), captured.clone(), bind.cloned());
        ok.connect_clicked(move |_| {
            let key = cap.borrow().clone();
            if key.is_empty() {
                return;
            }
            let idx = action_dd.selected() as usize;
            let real = match keep_idx {
                Some(_) if idx == 0 => {
                    // Keep-action sentinel: reuse the original verbatim.
                    if let Some(b) = &orig {
                        on_save(key, b.action.clone());
                    }
                    w.close();
                    return;
                }
                Some(_) => idx - 1,
                None => idx,
            };
            let (name, _, _) = ACTIONS[real];
            let v = reader.borrow().as_ref().map(|r| r()).unwrap_or_default();
            let mut attrs = Vec::new();
            match ACTIONS[real].2 {
                Arg::Command => {
                    if v.cmd.trim().is_empty() {
                        return;
                    }
                    attrs.push(("command".to_string(), v.cmd.trim().to_string()));
                }
                Arg::Direction => {
                    attrs.push(("direction".to_string(), DIRS[v.dir_idx].0.to_string()))
                }
                Arg::Desktop => {
                    attrs.push(("to".to_string(), v.desktop.to_string()));
                    if name == "SendToDesktop" {
                        attrs.push((
                            "follow".to_string(),
                            if v.follow { "yes" } else { "no" }.into(),
                        ));
                    }
                }
                Arg::Menu => attrs.push(("menu".to_string(), v.menu.trim().to_string())),
                Arg::None => {}
            }
            on_save(
                key,
                Action {
                    name: name.to_string(),
                    attrs,
                },
            );
            w.close();
        });
    }
    win.set_child(Some(&body));
    win.present();
}
