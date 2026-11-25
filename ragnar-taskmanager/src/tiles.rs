// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Vertical fill-tile sets — the uniform per-core/per-device visual
//! the original uses on every performance page: a boxed tile with a
//! name on top, a vertical fill bar in the middle (green <30 %,
//! amber <70 %, red above) and a value label below. Tiles are keyed
//! by device name and reused across ticks.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

/// One tile's live parts.
struct Tile {
    frac: Rc<RefCell<f64>>,
    area: gtk4::DrawingArea,
    val: gtk4::Label,
}

/// A FlowBox of named tiles; callers push (name, frac, text) updates.
pub struct TileSet {
    pub widget: gtk4::FlowBox,
    tiles: RefCell<Vec<(String, Tile)>>,
}

impl TileSet {
    /// Empty tile set in an 8-per-line flow grid.
    pub fn new() -> Self {
        let w = gtk4::FlowBox::new();
        w.set_max_children_per_line(8);
        w.set_column_spacing(8);
        w.set_row_spacing(8);
        w.set_selection_mode(gtk4::SelectionMode::None);
        Self { widget: w, tiles: RefCell::new(Vec::new()) }
    }

    /// Update `name`'s fill/value — creates the tile on first sight.
    pub fn set(&self, name: &str, frac: f64, text: &str) {
        if let Some((_, t)) = self.tiles.borrow().iter().find(|(n, _)| n == name) {
            *t.frac.borrow_mut() = frac;
            t.val.set_text(text);
            t.area.queue_draw();
            return;
        }
        let frac = Rc::new(RefCell::new(frac));
        let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        tile.add_css_class("card");
        tile.set_size_request(96, 190);
        tile.set_margin_top(6);
        tile.set_margin_bottom(6);
        let head = gtk4::Label::new(Some(name));
        let area = gtk4::DrawingArea::builder()
            .vexpand(true)
            .hexpand(true)
            .build();
        area.set_draw_func({
            let frac = frac.clone();
            move |_, cr, w, h| draw_tile(cr, w as f64, h as f64, *frac.borrow())
        });
        let val = gtk4::Label::new(Some(text));
        tile.append(&head);
        tile.append(&area);
        tile.append(&val);
        self.widget.append(&tile);
        self.tiles
            .borrow_mut()
            .push((name.to_string(), Tile { frac, area, val }));
    }

    /// Drop tiles whose names are not in `keep` (device unplugged etc.).
    pub fn retain(&self, keep: &[String]) {
        let mut t = self.tiles.borrow_mut();
        let dead: Vec<String> = t
            .iter()
            .filter(|(n, _)| !keep.contains(n))
            .map(|(n, _)| n.clone())
            .collect();
        for n in dead {
            if let Some(i) = t.iter().position(|(x, _)| x == &n) {
                let (_, tile) = t.remove(i);
                if let Some(p) = tile.area.parent().and_then(|w| w.parent()) {
                    self.widget.remove(&p);
                }
            }
        }
    }
}

/// Paint one vertical utilisation bar: green <30 %, amber <70 %,
/// red above — same thresholds as the original.
fn draw_tile(cr: &gtk4::cairo::Context, w: f64, h: f64, frac: f64) {
    let (r, g, bl) = if frac < 0.3 {
        (0.28, 0.68, 0.32)
    } else if frac < 0.7 {
        (0.95, 0.65, 0.15)
    } else {
        (0.86, 0.27, 0.24)
    };
    cr.set_source_rgba(0.9, 0.9, 0.9, 0.25);
    let _ = cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.fill();
    let fh = frac.clamp(0.0, 1.0) * h;
    cr.set_source_rgb(r, g, bl);
    let _ = cr.rectangle(0.0, h - fh, w, fh);
    let _ = cr.fill();
}
