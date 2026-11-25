// Copyright (C) Jörg Schröder
//! The "Erweitert" section of the firewall page — the full ufw rule
//! surface for power users. Simple mode stays untouched; expanding
//! this group reveals the numbered rule list with move/edit/delete
//! controls and the complete add dialog (action, direction,
//! interface, log, from/to, port, proto, comment, position).

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use adw::{ActionRow, ComboRow, EntryRow, ExpanderRow, PreferencesGroup};
use gtk4::{Align, Button, Image, ListBox, StringList};
use libadwaita as adw;

use crate::backend::{self, UfwRule, UfwStatus};

/// Build the collapsible advanced group for `status`.
/// `mutate`/`refresh` are the firewall page's worker plumbing —
/// the page hands them over so this module stays UI-only.
pub fn group(fw: Rc<RefCell<dyn FwOps>>, status: &UfwStatus) -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Advanced"))
        .build();

    let exp = ExpanderRow::builder()
        .title(gettextrs::gettext("Expert controls"))
        .subtitle(gettextrs::gettext(
            "Numbered rules, reordering and the full UFW rule syntax.",
        ))
        .build();
    group.add(&exp);

    // Numbered rules (as ufw sees them) with move up/down + edit.
    for rule in &status.rules {
        exp.add_row(&adv_rule_row(fw.clone(), rule, status));
    }

    let add = ActionRow::builder()
        .title(gettextrs::gettext("Add advanced rule…"))
        .activatable(true)
        .build();
    add.add_prefix(&Image::from_icon_name("list-add-symbolic"));
    {
        let fw = fw.clone();
        let page = group.clone();
        add.connect_activated(move |_| {
            let fw2 = fw.clone();
            present_adv_dialog(&page, None, move |spec| {
                fw2.borrow()
                    .mutate(Box::new(move || backend::ufw_add_rule_advanced(&spec)));
            });
        });
    }
    exp.add_row(&add);
    group
}

/// One numbered rule with ↑ ↓ ✎ 🗑 controls.
fn adv_rule_row(fw: Rc<RefCell<dyn FwOps>>, rule: &UfwRule, status: &UfwStatus) -> ActionRow {
    let row = ActionRow::builder()
        .title(format!("[{}] {}", rule.number, rule.to))
        .subtitle(format!("{} ← {}", rule.action, rule.from))
        .build();
    let n = rule.number;
    let max = status.rules.len() as u32;

    for (icon, tip, delta) in [
        ("go-up-symbolic", "Move up", -1i64),
        ("go-down-symbolic", "Move down", 1i64),
    ] {
        let b = Button::builder()
            .icon_name(icon)
            .valign(Align::Center)
            .tooltip_text(gettextrs::gettext(tip))
            .build();
        b.add_css_class("flat");
        let new_pos = (n as i64 + delta).clamp(1, max as i64) as u32;
        if new_pos == n {
            b.set_sensitive(false);
        } else {
            let (fw, st) = (fw.clone(), status.clone());
            b.connect_clicked(move |_| {
                let st2 = st.clone();
                fw.borrow()
                    .mutate(Box::new(move || backend::ufw_move_rule(n, new_pos, &st2)));
            });
        }
        row.add_suffix(&b);
    }

    let edit = Button::builder()
        .icon_name("document-edit-symbolic")
        .valign(Align::Center)
        .tooltip_text(gettextrs::gettext("Edit rule"))
        .build();
    edit.add_css_class("flat");
    {
        let (fw, r, row2) = (fw.clone(), rule.clone(), row.clone());
        edit.connect_clicked(move |_| {
            let fw2 = fw.clone();
            let pos = r.number;
            present_adv_dialog(&row2, Some(&r), move |mut spec| {
                spec.position = Some(pos);
                let st_n = pos;
                fw2.borrow().mutate(Box::new(move || {
                    backend::ufw_delete_rule(st_n)?;
                    backend::ufw_add_rule_advanced(&spec)
                }));
            });
        });
    }
    row.add_suffix(&edit);

    let del = Button::builder()
        .icon_name("user-trash-symbolic")
        .valign(Align::Center)
        .tooltip_text(gettextrs::gettext("Remove rule"))
        .build();
    del.add_css_class("flat");
    {
        let fw = fw.clone();
        del.connect_clicked(move |_| {
            fw.borrow()
                .mutate(Box::new(move || backend::ufw_delete_rule(n)));
        });
    }
    row.add_suffix(&del);
    row
}

/// The full rule dialog — `prefill` populates for editing. A real
/// window instead of an AlertDialog: the form is too long for the
/// alert widget's fixed narrow content box.
fn present_adv_dialog(
    parent: &impl IsA<gtk4::Widget>,
    prefill: Option<&UfwRule>,
    submit: impl Fn(backend::AdvRule) + 'static,
) {
    let win = adw::Window::builder()
        .title(gettextrs::gettext("Advanced rule"))
        .default_width(620)
        .default_height(560)
        .modal(true)
        .build();
    if let Some(toplevel) = parent
        .root()
        .and_then(|r| r.downcast::<gtk4::Window>().ok())
    {
        win.set_transient_for(Some(&toplevel));
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    win.set_content(Some(&view));

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.set_margin_top(12);
    list.set_margin_bottom(6);
    list.set_margin_start(12);
    list.set_margin_end(12);

    let p = prefill
        .map(backend::spec_from_parsed_pub)
        .unwrap_or_default();

    let action = ComboRow::builder()
        .title(gettextrs::gettext("Action"))
        .model(&StringList::new(&["allow", "deny", "reject", "limit"]))
        .selected(match p.action.as_str() {
            "deny" => 1,
            "reject" => 2,
            "limit" => 3,
            _ => 0,
        })
        .build();
    list.append(&action);

    let direction = ComboRow::builder()
        .title(gettextrs::gettext("Direction"))
        .model(&StringList::new(&[
            &gettextrs::gettext("incoming"),
            &gettextrs::gettext("outgoing"),
        ]))
        .selected(u32::from(p.direction == "out"))
        .build();
    list.append(&direction);

    let iface = EntryRow::builder()
        .title(gettextrs::gettext("Interface (optional)"))
        .text(&p.iface)
        .build();
    list.append(&iface);

    let from = EntryRow::builder()
        .title(gettextrs::gettext("From (optional)"))
        .text(&p.from)
        .build();
    list.append(&from);

    let to_addr = EntryRow::builder()
        .title(gettextrs::gettext("To address (optional)"))
        .text(&p.to_addr)
        .build();
    list.append(&to_addr);

    let port = EntryRow::builder()
        .title(gettextrs::gettext("Port (optional)"))
        .text(&p.port)
        .build();
    list.append(&port);

    let proto = ComboRow::builder()
        .title(gettextrs::gettext("Protocol"))
        .model(&StringList::new(&["TCP + UDP", "TCP", "UDP"]))
        .selected(match p.proto.as_str() {
            "tcp" => 1,
            "udp" => 2,
            _ => 0,
        })
        .build();
    list.append(&proto);

    let position = EntryRow::builder()
        .title(gettextrs::gettext("Insert position (optional)"))
        .text(p.position.map(|n| n.to_string()).unwrap_or_default())
        .build();
    list.append(&position);

    let comment = EntryRow::builder()
        .title(gettextrs::gettext("Comment (optional)"))
        .text(&p.comment)
        .build();
    list.append(&comment);

    let log = adw::SwitchRow::builder()
        .title(gettextrs::gettext("Log matching packets"))
        .active(p.log)
        .build();
    list.append(&log);

    // The expert form is long — scrollable body, action row at the
    // bottom like the launcher properties dialog.
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(&list)
        .build();
    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    btns.set_margin_top(8);
    btns.set_margin_bottom(12);
    btns.set_margin_start(12);
    btns.set_margin_end(12);
    let cancel = gtk4::Button::with_label(&gettextrs::gettext("Cancel"));
    let ok = gtk4::Button::with_label(&gettextrs::gettext("Apply"));
    ok.add_css_class("suggested-action");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    btns.append(&cancel);
    btns.append(&ok);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    body.append(&scroll);
    body.append(&btns);
    view.set_content(Some(&body));

    let submit = Rc::new(submit);
    let w = win.clone();
    ok.connect_clicked(move |_| {
        submit(backend::AdvRule {
            action: ["allow", "deny", "reject", "limit"][action.selected() as usize].to_string(),
            direction: if direction.selected() == 1 {
                "out"
            } else {
                "in"
            }
            .into(),
            iface: iface.text().to_string(),
            from: from.text().to_string(),
            to_addr: to_addr.text().to_string(),
            port: port.text().to_string(),
            proto: match proto.selected() {
                1 => "tcp".into(),
                2 => "udp".into(),
                _ => "any".into(),
            },
            log: log.is_active(),
            comment: comment.text().to_string(),
            position: position
                .text()
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0),
        });
        w.close();
    });
    win.present();
}

/// Operations the firewall page provides to this module — keeps the
/// advanced UI decoupled from the page internals.
pub trait FwOps {
    fn mutate(&self, op: Box<dyn FnOnce() -> anyhow::Result<()> + Send + 'static>);
}
