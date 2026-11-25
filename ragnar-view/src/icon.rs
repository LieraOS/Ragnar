// Copyright (C) Jörg Schröder
//! One desktop icon: widget, click handling and drag & drop.

use crate::desktop::Surface;
use crate::items::{self, Item};
use crate::layout::CELL_H;
use gtk4::prelude::*;
use std::rc::Rc;

impl Surface {
    /// Build the icon for `it`: click selects, double-click opens,
    /// right-click shows the file menu, dragging moves it.
    pub(crate) fn icon_widget(self: &Rc<Self>, it: &Item) -> gtk4::Widget {
        let v = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        v.add_css_class("desktop-icon");
        v.set_size_request(96, CELL_H - 8);
        let img = gtk4::Image::from_gicon(&it.icon);
        img.set_pixel_size(48);
        img.set_margin_top(8);
        let lbl = crate::fxlabel::FxLabel::new(&it.name);
        lbl.set_wrap(true);
        lbl.set_shadow(true);
        lbl.set_max_width_chars(13);
        lbl.set_lines(2);
        lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        lbl.set_justify(gtk4::pango::Alignment::Center);
        v.append(&img);
        v.append(&lbl);
        self.attach_click(&v, it);
        v.upcast()
    }

    /// Click gesture: select / open / context menu.
    fn attach_click(self: &Rc<Self>, v: &gtk4::Box, it: &Item) {
        let click = gtk4::GestureClick::new();
        click.set_button(0);
        let (s, it) = (self.clone(), it.clone());
        click.connect_pressed(move |g, n, x, y| {
            let Some(w) = g.widget() else { return };
            // Window coordinates via the widget's own root — after a
            // cross-monitor drag its gestures still hold the origin
            // surface, which may be another monitor's window.
            let root_w = w.ancestor(gtk4::Window::static_type());
            let at = root_w.as_ref().and_then(|rw| {
                w.compute_point(rw, &gtk4::graphene::Point::new(x as f32, y as f32))
                    .map(|p| (p.x() as f64, p.y() as f64))
            });
            match g.current_button() {
                1 => {
                    // An already-selected icon keeps the group —
                    // dragging it moves every selected icon.
                    if !s.sel.borrow().contains(&it.id()) {
                        s.select(&w);
                    }
                    if let Some(p) = at {
                        s.begin_drag(&it.id(), &w, p);
                    }
                    if n == 2 {
                        items::open(&it);
                    }
                }
                3 => {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    s.select(&w);
                    if let (Some(rw), Some(p)) = (root_w, at) {
                        crate::menu_file::file(&s, &rw, p, &it);
                    }
                }
                _ => {}
            }
        });
        v.add_controller(click);
    }
}
