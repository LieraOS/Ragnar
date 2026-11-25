// Copyright (C) Jörg Schröder
// Layout chip strip for the bar settings page — a miniature of the
// real bar: applet chips are buttons, gaps render as dashed spring
// segments that stretch exactly like the bar's expanding spacers.
// Chips reorder via drag & drop, "+" slots between chips insert a
// new gap at that spot, clicking a gap edits its width
// (0 = flexible) or removes it.
use super::bar_page::{commit, APPLETS};
use super::Ui;
use gtk4::gdk;
use gtk4::prelude::*;
use std::rc::Rc;

/// The arrangement strip: alternating "+" gap slots and one chip per
/// layout entry, stretched like the real bar so the user edits what
/// the result actually looks like.
pub fn strip(ui: &Rc<Ui>, index: usize, cfg: &crate::config::BarConfig) -> gtk4::Widget {
    let strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    strip.add_css_class("layout-preview");
    strip.set_hexpand(true);
    strip.set_size_request(-1, 34);
    // "+" slot before every chip plus one trailing — a gap lands
    // exactly where the user is looking, not at the list's end.
    strip.append(&gap_slot(ui, index, 0));
    for (i, item) in cfg.layout.iter().enumerate() {
        strip.append(&chip(ui, index, item, i));
        strip.append(&gap_slot(ui, index, i + 1));
    }
    strip.upcast()
}

/// Small "+" between chips — inserts a gap at `pos` and doubles as a
/// drop target so dropping a chip between two others lands it there.
fn gap_slot(ui: &Rc<Ui>, index: usize, pos: usize) -> gtk4::Widget {
    let add = gtk4::Button::with_label("+");
    add.add_css_class("layout-gap-slot");
    add.set_tooltip_text(Some(&crate::i18n::tr("Abstand einfügen")));
    let u = ui.clone();
    add.connect_clicked(move |_| {
        commit(index, |c| {
            c.layout.insert(pos.min(c.layout.len()), "spacer".into());
        });
        super::rebuild_all(&u);
    });
    let dt = gtk4::DropTarget::new(i32::static_type(), gdk::DragAction::MOVE);
    let u = ui.clone();
    dt.connect_drop(move |_, val, _, _| {
        let Ok(from) = val.get::<i32>() else {
            return false;
        };
        let mut cfg = crate::bars::bars().borrow()[index]
            .state
            .borrow()
            .cfg
            .clone();
        let from = from as usize;
        if from >= cfg.layout.len() {
            return false;
        }
        let item = cfg.layout.remove(from);
        // Removing an entry before the slot shifts the slot left.
        let at = (if from < pos { pos - 1 } else { pos }).min(cfg.layout.len());
        cfg.layout.insert(at, item);
        crate::bars::commit(index, cfg);
        super::rebuild_all(&u);
        true
    });
    add.add_controller(dt);
    add.upcast()
}

/// One layout chip: label + drag source + drop target for reordering.
/// Gap chips get the dashed "spring" look — flexible ones expand
/// like on the bar, fixed ones keep their pixel width; clicking
/// opens the width popover with a remove button.
fn chip(ui: &Rc<Ui>, index: usize, item: &str, pos: usize) -> gtk4::Widget {
    let px = item
        .strip_prefix("spacer:")
        .and_then(|n| n.parse::<i32>().ok());
    let label = APPLETS
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, n)| crate::i18n::tr(n))
        .unwrap_or_else(|| match px {
            Some(p) => format!("{}px", p),
            None if item == "spacer" => "⇆".into(),
            _ => item.to_string(),
        });
    let chip = gtk4::Button::with_label(&label);
    chip.add_css_class("layout-chip");

    if item.starts_with("spacer") {
        chip.add_css_class("layout-spacer");
        chip.set_tooltip_text(Some(&crate::i18n::tr(
            "Abstand — klicken für Breite, ziehen zum Verschieben",
        )));
        // Mirror the bar: flexible gaps stretch, fixed gaps keep
        // their pixel width (clamped so they stay clickable).
        if let Some(p) = px {
            chip.set_size_request(p.clamp(26, 800), -1);
        } else {
            chip.set_hexpand(true);
        }
        // Gap click → width popover (0 = flexible) + remove.
        let pop = gtk4::Popover::new();
        pop.set_parent(&chip);
        let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let spin = gtk4::SpinButton::with_range(0.0, 800.0, 5.0);
        spin.set_value(px.unwrap_or(0) as f64);
        spin.set_tooltip_text(Some(&crate::i18n::tr("0 = flexibel")));
        bx.append(&spin);
        let rm = gtk4::Button::with_label(&crate::i18n::tr("Entfernen"));
        {
            let (u, p) = (ui.clone(), pop.clone());
            rm.connect_clicked(move |_| {
                commit(index, |c| {
                    if pos < c.layout.len() {
                        c.layout.remove(pos);
                    }
                });
                p.popdown();
                super::rebuild_all(&u);
            });
        }
        bx.append(&rm);
        pop.set_child(Some(&bx));
        {
            spin.connect_value_changed(move |s| {
                let v = s.value() as i32;
                commit(index, |c| {
                    if pos < c.layout.len() {
                        c.layout[pos] = if v == 0 {
                            "spacer".into()
                        } else {
                            format!("spacer:{v}")
                        };
                    }
                });
            });
            let u = ui.clone();
            pop.connect_closed(move |_| super::rebuild_all(&u));
        }
        let p = pop.clone();
        chip.connect_clicked(move |_| p.popup());
    }

    // Reorder by drag & drop — payload is the source position.
    let drag = gtk4::DragSource::new();
    drag.set_actions(gdk::DragAction::MOVE);
    let from = pos as i32;
    drag.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&from.to_value())));
    chip.add_controller(drag);

    let dt = gtk4::DropTarget::new(i32::static_type(), gdk::DragAction::MOVE);
    let u = ui.clone();
    dt.connect_drop(move |_, val, _, _| {
        let Ok(from) = val.get::<i32>() else {
            return false;
        };
        let mut cfg = crate::bars::bars().borrow()[index]
            .state
            .borrow()
            .cfg
            .clone();
        let item = cfg.layout.remove(from as usize);
        cfg.layout.insert(pos.min(cfg.layout.len()), item);
        crate::bars::commit(index, cfg);
        super::rebuild_all(&u);
        true
    });
    chip.add_controller(dt);
    chip.upcast()
}
