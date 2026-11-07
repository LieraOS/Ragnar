// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Keyboard shortcuts" panel
// Copyright (C) 2026 LieraOS Project
//
// Top section lists the compositor's existing <keybind> entries
// from rc.xml read-only — the real, working bindings. The bottom
// section manages user-defined bindings: capture a key combo,
// attach a command, append a <keybind> block, reload labwc.
// User bindings are tagged with an XML comment marker so they can
// be removed without ever touching system bindings.

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, ListBox, Orientation};
use std::path::PathBuf;

const MARKER: &str = "<!-- ragnar-settings:user -->";

fn rc_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-desktop");
    p.push("labwc");
    p.push("rc.xml");
    p
}

/// One parsed keybind.
struct Bind {
    key: String,
    cmd: String,
    user: bool,
}

/// Parse all `<keybind>` blocks from rc.xml.
fn keybinds() -> Vec<Bind> {
    let t = std::fs::read_to_string(rc_path()).unwrap_or_default();
    let mut out = Vec::new();
    let mut rest = t.as_str();
    while let Some(pos) = rest.find("<keybind") {
        let seg = &rest[pos..];
        let Some(end) = seg
            .find("</keybind>")
            .map(|i| i + "</keybind>".len())
            .or_else(|| seg.find("/>").map(|i| i + 2))
        else {
            break;
        };
        let block = &seg[..end];
        let key = attr(block, "key").unwrap_or_default();
        let cmd = attr(block, "command").unwrap_or_default();
        let was_user = rest[..pos]
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .contains(MARKER);
        if !key.is_empty() {
            out.push(Bind {
                key,
                cmd,
                user: was_user,
            });
        }
        rest = &seg[end..];
    }
    out
}

/// Extract attr="value" from a tag block.
fn attr(block: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let a = block.find(&pat)? + pat.len();
    let b = block[a..].find('"')?;
    Some(block[a..a + b].to_string())
}

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
fn human_key(key: &str) -> String {
    let (c, a, s, w) = mod_names();
    let mut out = Vec::new();
    let mut rest = key;
    // Leading modifier letters (W/C/A/S) before the key name.
    while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
        let m = &rest[..1];
        out.push(match m {
            "C" => c,
            "A" => a,
            "S" => s,
            "W" => w,
            _ => m,
        });
        rest = &rest[2..];
    }
    out.push(rest);
    out.join("+")
}

/// Raw command → readable description. Strips the dbus-send
/// plumbing and names the action instead.
fn describe(cmd: &str) -> String {
    let known: &[(&str, &str)] = &[
        ("MprisController.PlayPause", "Media: Play/Pause"),
        ("MprisController.Play", "Media: Play"),
        ("MprisController.Next", "Media: Next track"),
        ("MprisController.Previous", "Media: Previous track"),
        ("MprisController.Stop", "Media: Stop"),
        ("ShowMenu", "Show application menu"),
        ("org.lieraos.ragnar.Screenshot", "Take a screenshot"),
        ("org.lieraos.ragnar.PowerDialog", "Show power dialog"),
        ("org.lieraos.ragnar.Lockscreen", "Lock the screen"),
        ("org.lieraos.ragnar.Screenlock", "Lock the screen"),
    ];
    for (pat, label) in known {
        if cmd.contains(pat) {
            return gettext(*label);
        }
    }
    // Fallback: last meaningful token of the command.
    cmd.split_whitespace()
        .filter(|t| !t.starts_with('-'))
        .last()
        .unwrap_or(cmd)
        .to_string()
}

/// Build the panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    // -- System bindings (read-only) ---------------------------------------
    let sys_h = gtk4::Label::new(Some(&gettext("System shortcuts")));
    sys_h.add_css_class("heading");
    sys_h.set_xalign(0.0);
    page.append(&sys_h);
    let sys = ListBox::new();
    sys.set_selection_mode(gtk4::SelectionMode::None);
    sys.add_css_class("boxed-list");
    for b in keybinds().iter().filter(|b| !b.user) {
        sys.append(&bind_row(&b.key, &b.cmd, None));
    }
    page.append(&sys);

    // -- User bindings -----------------------------------------------------
    let usr_h = Box::new(Orientation::Horizontal, 10);
    let hl = gtk4::Label::new(Some(&gettext("Custom shortcuts")));
    hl.add_css_class("heading");
    hl.set_xalign(0.0);
    hl.set_hexpand(true);
    usr_h.append(&hl);
    let add = gtk4::Button::with_label(&gettext("Add…"));
    usr_h.append(&add);
    page.append(&usr_h);

    let usr = ListBox::new();
    usr.set_selection_mode(gtk4::SelectionMode::None);
    usr.add_css_class("boxed-list");
    page.append(&usr);
    fill_user(&usr);

    let u = usr.clone();
    add.connect_clicked(move |b| add_dialog(b, &u));

    let note = gtk4::Label::new(Some(&gettext(
        "Custom bindings run a command on key press. They take \
         effect immediately after applying.",
    )));
    note.add_css_class("dim-label");
    note.set_xalign(0.0);
    note.set_wrap(true);
    page.append(&note);

    page.upcast()
}

/// Re-render the user-bindings list.
fn fill_user(list: &ListBox) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    for b in keybinds().iter().filter(|b| b.user) {
        let rm = gtk4::Button::from_icon_name("user-trash-symbolic");
        rm.set_has_frame(false);
        let (key, list2) = (b.key.clone(), list.clone());
        rm.connect_clicked(move |_| {
            remove_binding(&key);
            fill_user(&list2);
        });
        list.append(&bind_row(&b.key, &b.cmd, Some(rm.upcast())));
    }
}

/// Display row: key combo (mono) + command, optional trailing widget.
fn bind_row(key: &str, cmd: &str, extra: Option<gtk4::Widget>) -> gtk4::ListBoxRow {
    let inner = Box::new(Orientation::Horizontal, 14);
    for m in ["top", "bottom", "start", "end"] {
        inner.set_property(&format!("margin-{m}"), 10);
    }
    // Key combo rendered as a small keyboard-style chip.
    let chip = gtk4::Frame::new(None);
    chip.add_css_class("view");
    let k = gtk4::Label::new(Some(&human_key(key)));
    k.set_margin_start(8);
    k.set_margin_end(8);
    k.set_margin_top(3);
    k.set_margin_bottom(3);
    k.set_valign(gtk4::Align::Center);
    chip.set_child(Some(&k));
    chip.set_valign(gtk4::Align::Center);
    inner.append(&chip);

    let c = gtk4::Label::new(Some(&describe(cmd)));
    c.set_xalign(0.0);
    c.set_hexpand(true);
    c.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    c.set_valign(gtk4::Align::Center);
    inner.append(&c);
    if let Some(w) = extra {
        inner.append(&w);
    }
    let r = gtk4::ListBoxRow::new();
    r.set_child(Some(&inner));
    r
}

/// Append a user keybind + reload the compositor.
fn add_binding(key: &str, cmd: &str) {
    let mut t = std::fs::read_to_string(rc_path()).unwrap_or_default();
    let Some(pos) = t.find("</keyboard>") else {
        return;
    };
    let block = format!(
        "\t\t{MARKER}\n\t\t<keybind key=\"{key}\"><action name=\"Execute\" command=\"{cmd}\" /></keybind>\n"
    );
    t.insert_str(pos, &block);
    let _ = std::fs::write(rc_path(), t);
    let _ = std::process::Command::new("labwc").arg("-r").status();
}

/// Delete the user's binding: its block plus the marker comment
/// line that precedes it.
fn remove_binding(key: &str) {
    let mut t = std::fs::read_to_string(rc_path()).unwrap_or_default();
    let pat = format!("<keybind key=\"{key}\"");
    if let Some(a) = t.find(&pat) {
        let start = t[..a].rfind(MARKER).unwrap_or(a);
        if let Some(rel) = t[a..].find("</keybind>") {
            t.drain(start..a + rel + "</keybind>\n".len());
            let _ = std::fs::write(rc_path(), t);
            let _ = std::process::Command::new("labwc").arg("-r").status();
        }
    }
}

/// "Add" dialog — a key-grab button captures the combo, an entry
/// holds the command.
fn add_dialog(anchor: &gtk4::Button, list: &ListBox) {
    let win = gtk4::Window::builder()
        .title(gettext("Add shortcut"))
        .modal(true)
        .default_width(380)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 12);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }

    let grab = gtk4::Button::with_label(&gettext("Press shortcut…"));
    body.append(&grab);
    let cmd_e = gtk4::Entry::builder()
        .placeholder_text(gettext("Command to run, e.g. firefox"))
        .build();
    body.append(&cmd_e);
    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let ok = gtk4::Button::with_label(&gettext("Apply"));
    ok.add_css_class("suggested-action");
    btns.append(&cancel);
    btns.append(&ok);
    body.append(&btns);

    // Key capture: modifiers + key name in labwc notation.
    let captured = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let keyc = gtk4::EventControllerKey::new();
    keyc.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let (g, cap) = (grab.clone(), captured.clone());
    keyc.connect_key_pressed(move |_, kv, _, state| {
        let mut mods = String::new();
        if state.contains(gtk4::gdk::ModifierType::SUPER_MASK) {
            mods.push('W');
        }
        if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            mods.push_str("C-");
        }
        if state.contains(gtk4::gdk::ModifierType::ALT_MASK) {
            mods.push_str("A-");
        }
        if state.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
            mods.push_str("S-");
        }
        let Some(name) = kv.name() else {
            return gtk4::glib::Propagation::Proceed;
        };
        if !mods.is_empty() || !name.to_lowercase().contains("shift") {
            let combo = format!("{mods}{name}");
            *cap.borrow_mut() = combo.clone();
            g.set_label(&human_key(&combo));
        }
        gtk4::glib::Propagation::Stop
    });
    win.add_controller(keyc);
    grab.connect_clicked(|b| b.set_label(&gettext("Press shortcut…")));

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    {
        let (w, l, c) = (win.clone(), list.clone(), cmd_e.clone());
        ok.connect_clicked(move |_| {
            let (key, cmd) = (captured.borrow().clone(), c.text().trim().to_string());
            if !key.is_empty() && !cmd.is_empty() {
                add_binding(&key, &cmd);
                fill_user(&l);
            }
            w.close();
        });
    }
    win.set_child(Some(&body));
    win.present();
}
