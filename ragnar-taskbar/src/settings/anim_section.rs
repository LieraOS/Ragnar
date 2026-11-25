// Copyright (C) Jörg Schröder
// Animation section of the per-bar settings page — the animated
// gradient background: on/off switch, a small colour-list editor,
// flow direction, speed and the smoothness (fps) preset. Everything
// is per bar and commits live like every other section.
use super::bar_page::{commit, head, slider};
use super::Ui;
use crate::config::BarConfig;
use gtk4::prelude::*;
use std::rc::Rc;

/// Direction ids ↔ dropdown order.
const DIRS: &[(&str, &str)] = &[
    ("ltr", "Rechts"),
    ("rtl", "Links"),
    ("down", "Unten"),
    ("up", "Oben"),
    ("spin", "Rotierend"),
];

/// Smoothness presets ↔ repaint rate.
const FPS: &[(&str, i32)] = &[
    ("Sparsam (15 FPS)", 15),
    ("Normal (30 FPS)", 30),
    ("Flüssig (60 FPS)", 60),
];

/// The whole animation section — a master switch plus the option
/// rows, insensitive while the animation is off.
pub fn section(ui: &Rc<Ui>, index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let sec = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    sec.append(&head(&crate::i18n::tr("Animation")));

    let sub = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    sub.set_sensitive(cfg.anim);

    // Master switch — toggling flips the sub-rows' sensitivity
    // directly; the bar side reacts through the live commit.
    let srow = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let l = gtk4::Label::new(Some(&crate::i18n::tr("Aktiviert")));
    l.set_halign(gtk4::Align::Start);
    l.set_hexpand(true);
    srow.append(&l);
    let sw = gtk4::Switch::new();
    sw.set_active(cfg.anim);
    {
        let sub = sub.clone();
        sw.connect_active_notify(move |s| {
            let on = s.is_active();
            commit(index, |c| c.anim = on);
            sub.set_sensitive(on);
        });
    }
    srow.append(&sw);
    sec.append(&srow);

    sub.append(&colors_row(ui, index, cfg));
    sub.append(&direction_row(index, cfg));
    sub.append(&slider(
        index,
        &crate::i18n::tr("Tempo"),
        1.0,
        10.0,
        cfg.anim_speed as f64,
        |c, v| c.anim_speed = v as i32,
    ));
    sub.append(&smooth_row(index, cfg));
    sec.append(&sub);
    sec.upcast()
}

/// Colour list editor — one picker per colour plus a remove button
/// (kept at a two-colour minimum) and an add button (capped at 8).
fn colors_row(ui: &Rc<Ui>, index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let l = gtk4::Label::new(Some(&crate::i18n::tr("Farben")));
    l.set_halign(gtk4::Align::Start);
    l.set_size_request(90, -1);
    row.append(&l);
    let list = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let many = cfg.anim_colors.len() > 2;
    for (i, hex) in cfg.anim_colors.iter().enumerate() {
        let cell = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        let dialog = gtk4::ColorDialog::builder().with_alpha(false).build();
        let pick = gtk4::ColorDialogButton::new(Some(dialog));
        if let Ok(c) = gtk4::gdk::RGBA::parse(hex) {
            pick.set_rgba(&c);
        }
        pick.connect_rgba_notify(move |b| {
            let c = b.rgba();
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (c.red() * 255.0) as u8,
                (c.green() * 255.0) as u8,
                (c.blue() * 255.0) as u8
            );
            commit(index, move |cfg| {
                if i < cfg.anim_colors.len() {
                    cfg.anim_colors[i] = hex;
                }
            });
        });
        cell.append(&pick);
        if many {
            let rm = gtk4::Button::with_label("×");
            rm.set_has_frame(false);
            let u = ui.clone();
            rm.connect_clicked(move |_| {
                commit(index, |c| {
                    if c.anim_colors.len() > 2 && i < c.anim_colors.len() {
                        c.anim_colors.remove(i);
                    }
                });
                super::rebuild_all(&u);
            });
            cell.append(&rm);
        }
        list.append(&cell);
    }
    let add = gtk4::Button::with_label(&crate::i18n::tr("+ Farbe"));
    if cfg.anim_colors.len() >= 8 {
        add.set_sensitive(false);
    }
    let u = ui.clone();
    add.connect_clicked(move |_| {
        commit(index, |c| {
            if c.anim_colors.len() < 8 {
                let last = c.anim_colors.last().cloned().unwrap_or("#ffffff".into());
                c.anim_colors.push(last);
            }
        });
        super::rebuild_all(&u);
    });
    list.append(&add);
    row.append(&list);
    row.upcast()
}

/// Flow direction dropdown — ids map to the anim_dir config key.
fn direction_row(index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let l = gtk4::Label::new(Some(&crate::i18n::tr("Richtung")));
    l.set_halign(gtk4::Align::Start);
    l.set_size_request(90, -1);
    row.append(&l);
    let names: Vec<String> = DIRS.iter().map(|(_, n)| crate::i18n::tr(n)).collect();
    let dd = gtk4::DropDown::from_strings(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let cur = DIRS
        .iter()
        .position(|(id, _)| *id == cfg.anim_dir)
        .unwrap_or(0);
    dd.set_selected(cur as u32);
    dd.connect_selected_notify(move |d| {
        let dir = DIRS[d.selected() as usize].0.to_string();
        commit(index, move |c| c.anim_dir = dir.clone());
    });
    row.append(&dd);
    row.upcast()
}

/// Smoothness dropdown — the repaint-rate preset.
fn smooth_row(index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let l = gtk4::Label::new(Some(&crate::i18n::tr("Glätte")));
    l.set_halign(gtk4::Align::Start);
    l.set_size_request(90, -1);
    row.append(&l);
    let names: Vec<String> = FPS.iter().map(|(n, _)| crate::i18n::tr(n)).collect();
    let dd = gtk4::DropDown::from_strings(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let cur = FPS
        .iter()
        .position(|(_, f)| *f == cfg.anim_fps)
        .unwrap_or(1);
    dd.set_selected(cur as u32);
    dd.connect_selected_notify(move |d| {
        let fps = FPS[d.selected() as usize].1;
        commit(index, move |c| c.anim_fps = fps);
    });
    row.append(&dd);
    row.upcast()
}
