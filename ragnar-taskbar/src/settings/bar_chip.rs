// Layout chip strip for the bar settings page — one draggable chip
// per layout entry. Chips reorder the bar's applet sequence via
// drag & drop; spacer chips additionally open a width popover on
// click (0 = flexible) with a remove button.
use super::bar_page::{commit, APPLETS};
use super::Ui;
use gtk4::gdk;
use gtk4::prelude::*;
use std::rc::Rc;

/// One layout chip: label + drag source + drop target for reordering;
/// spacer chips additionally open a width popover on click.
pub fn chip(ui: &Rc<Ui>, index: usize, item: &str, pos: usize) -> gtk4::Widget {
    let px = item
        .strip_prefix("spacer:")
        .and_then(|n| n.parse::<i32>().ok());
    let label = APPLETS
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, n)| crate::i18n::tr(n))
        .unwrap_or_else(|| match px {
            Some(p) => format!("⇔{}px", p),
            None if item == "spacer" => "⇔".into(),
            _ => item.to_string(),
        });
    let chip = gtk4::Button::with_label(&label);
    chip.add_css_class("layout-chip");

    if item.starts_with("spacer") {
        // Spacer click → width popover (0 = flexible) + remove.
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
