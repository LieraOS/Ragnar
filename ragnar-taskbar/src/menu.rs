// Copyright (C) Jörg Schröder
// Menu primitive for layer-shell bars.
//
// Design (rewritten after the two-surface shield approach proved
// unreliable under labwc): a menu is a SINGLE fullscreen layer-shell
// surface on Layer::Overlay, transparent, that carries the menu box
// as a child. The box positions itself through plain GTK layout —
// halign=Start, valign=End plus margins — so no manual measuring or
// coordinate math is involved: the menu bottom sits a few px above
// the docked bar no matter how tall the menu content is. Any click
// on the surface closes the menu unless `pick()` reports it hit a
// widget actually inside the menu box. One surface means one input
// stream, no grab fights, no leftover windows. Only one menu is
// ever open at a time.
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

thread_local! {
    static OPEN: std::cell::RefCell<Option<gtk4::Window>> =
        const { std::cell::RefCell::new(None) };
}

/// Close the currently open menu, if any. Idempotent.
pub fn close_current() {
    // Take the window out BEFORE closing — the borrow_mut guard
    // must be dead by then, because unmapping fires leave handlers
    // that call close() again (RefCell double-borrow → panic).
    let w = OPEN.with(|o| o.borrow_mut().take());
    if let Some(w) = w {
        w.close();
    }
}

/// Build the fullscreen overlay surface that hosts the menu.
/// `widget_left`/`widget_right`/`surf_w` are the applet's edges and
/// the bar surface's width — all in bar-surface coordinates, so the
/// overlay (which covers the same output) can anchor the menu to the
/// WIDGET, not the pointer position.
pub fn shell(widget_left: f64, widget_right: f64, surf_w: f64, bar_height: i32) -> gtk4::Window {
    let (win, menu_box) = base_shell();
    position_bottom(&menu_box, widget_left, widget_right, surf_w, bar_height);
    win
}

/// Edge-aware variant: the menu opens against the applet in whichever
/// direction the bar's edge dictates — above for bottom bars, below
/// for top bars, beside for vertical (left/right) bars.
pub fn shell_at(widget: &impl IsA<gtk4::Widget>, edge: &str) -> gtk4::Window {
    let (l, t, r, b, sw, sh, bw, bh) = widget_rect(widget);
    let (win, menu_box) = base_shell();
    match edge {
        "top" => position_top(&menu_box, l, r, sw, bh),
        "left" => position_side(&menu_box, t, b, sh, bw, true),
        "right" => position_side(&menu_box, t, b, sh, bw, false),
        _ => position_bottom(&menu_box, l, r, sw, bh),
    }
    win
}

/// The overlay window plus its inner menu box — all the shared
/// machinery (single OPEN slot, outside-click + ESC dismissal,
/// transparent fullscreen surface) without any positioning yet.
fn base_shell() -> (gtk4::Window, gtk4::Box) {
    // Tear down any previously open menu — no stacking.
    close_current();

    let win = gtk4::Window::builder().decorated(false).build();
    win.add_css_class("menu-overlay");
    win.init_layer_shell();
    win.set_layer(Layer::Overlay);
    // The overlay owns the keyboard while open so ESC works.
    win.set_keyboard_mode(KeyboardMode::Exclusive);
    // Cover the whole monitor — outside clicks land on this surface.
    // exclusive_zone(-1): ignore the bar's reserved zone so the
    // surface truly spans the full output; otherwise labwc shrinks
    // it by the bar height and every menu floats one bar-height too
    // high above the taskbar.
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        win.set_anchor(e, true);
    }
    win.set_exclusive_zone(-1);

    // Overlay container: every menu box (root plus any cascading
    // submenu) gets a full-surface cell and positions itself via
    // halign/valign + margins — one input surface, no grab fights.
    let fill = gtk4::Overlay::new();
    fill.set_hexpand(true);
    fill.set_vexpand(true);
    let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    menu_box.add_css_class("menu-box");
    // Expand so the box's cell spans the whole surface — halign /
    // valign can only position a widget INSIDE its allocated cell;
    // without expand the cell is just the natural size at box start.
    menu_box.set_hexpand(true);
    menu_box.set_vexpand(true);
    fill.set_child(Some(&menu_box));
    win.set_child(Some(&fill));

    // All live menu boxes on this surface — the outside-click test
    // accepts a pick inside ANY of them (root or a cascade level).
    let boxes: std::rc::Rc<std::cell::RefCell<Vec<gtk4::Box>>> =
        std::rc::Rc::new(std::cell::RefCell::new(vec![menu_box.clone()]));

    // Outside-click dismissal: close unless the picked widget is the
    // menu box itself or a real descendant of it (identity, not
    // type — the overlay has other Box widgets too).
    let click = gtk4::GestureClick::new();
    click.set_button(0);
    {
        let boxes_ref = boxes.clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let picked = gesture
                .widget()
                .and_then(|w| w.pick(x, y, gtk4::PickFlags::DEFAULT));
            // p.is_ancestor(b) means "p is somewhere inside b" —
            // the inside-test is "the pick hit a menu box or one of
            // its descendants", checked against EVERY live box.
            let inside = picked
                .as_ref()
                .map(|p| {
                    boxes_ref
                        .borrow()
                        .iter()
                        .any(|b| *p == *b || p.is_ancestor(b))
                })
                .unwrap_or(false);
            if !inside {
                close_current();
            }
        });
    }
    win.add_controller(click);

    // ESC dismisses the menu.
    let esc = gtk4::EventControllerKey::new();
    esc.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            close_current();
            return gtk4::glib::Propagation::Stop;
        }
        gtk4::glib::Propagation::Proceed
    });
    win.add_controller(esc);

    // Stash the containers `show()`/`submenu()` need later.
    unsafe { win.set_data("menu_box", menu_box.clone()) };
    unsafe { win.set_data("overlay", fill.clone()) };
    unsafe { win.set_data("menu_boxes", boxes) };
    OPEN.with(|o| *o.borrow_mut() = Some(win.clone()));
    (win, menu_box)
}

/// Pin the menu box just above the applet, edge-to-edge — the
/// classic bottom-bar anchoring. Right-half applets pin the menu's
/// RIGHT edge so wide panels can never overflow the screen.
fn position_bottom(menu_box: &gtk4::Box, left: f64, right: f64, sw: f64, bh: i32) {
    let mon_w = sw.max(1.0) as i32;
    menu_box.set_valign(gtk4::Align::End);
    menu_box.set_margin_bottom(bh + 4);
    if left as i32 > mon_w / 2 {
        menu_box.set_halign(gtk4::Align::End);
        menu_box.set_margin_end((mon_w - right.min(sw) as i32).max(0));
    } else {
        menu_box.set_halign(gtk4::Align::Start);
        menu_box.set_margin_start(left.min(sw).max(0.0) as i32);
    }
}

/// Top-bar mirror of `position_bottom` — menu drops BELOW the bar.
fn position_top(menu_box: &gtk4::Box, left: f64, right: f64, sw: f64, bh: i32) {
    let mon_w = sw.max(1.0) as i32;
    menu_box.set_valign(gtk4::Align::Start);
    menu_box.set_margin_top(bh + 4);
    if left as i32 > mon_w / 2 {
        menu_box.set_halign(gtk4::Align::End);
        menu_box.set_margin_end((mon_w - right.min(sw) as i32).max(0));
    } else {
        menu_box.set_halign(gtk4::Align::Start);
        menu_box.set_margin_start(left.min(sw).max(0.0) as i32);
    }
}

/// Vertical-bar anchoring — the menu sits beside the bar (right of a
/// left edge bar, left of a right edge bar), vertically pinned at the
/// applet's end like the horizontal edges pin horizontally.
fn position_side(menu_box: &gtk4::Box, top: f64, bottom: f64, sh: f64, bw: i32, is_left: bool) {
    let mon_h = sh.max(1.0) as i32;
    if is_left {
        menu_box.set_halign(gtk4::Align::Start);
        menu_box.set_margin_start(bw + 4);
    } else {
        menu_box.set_halign(gtk4::Align::End);
        menu_box.set_margin_end(bw + 4);
    }
    if top as i32 > mon_h / 2 {
        // Lower-half applet: pin the menu's BOTTOM edge to the
        // widget's bottom edge so tall menus stay on-screen.
        menu_box.set_valign(gtk4::Align::End);
        menu_box.set_margin_bottom((mon_h - bottom.min(sh) as i32).max(0));
    } else {
        menu_box.set_valign(gtk4::Align::Start);
        menu_box.set_margin_top(top.min(sh).max(0.0) as i32);
    }
}

/// Pack the menu content into the overlay's menu box, clamp its
/// left margin against the real menu width so nothing hangs off
/// the right screen edge, then present.
pub fn show(win: &gtk4::Window, content: &impl IsA<gtk4::Widget>) {
    let menu_box = unsafe { win.data::<gtk4::Box>("menu_box") }
        .map(|p| unsafe { p.as_ref() }.clone())
        .expect("menu_box stashed in shell()");
    menu_box.append(content);
    win.present();
}

/// Open a cascading submenu box beside `anchor` (the row that was
/// clicked) on the SAME overlay surface — real submenus, no second
/// surface, dismissed together with the root menu. Returns the box
/// to fill, or None when no menu surface is open.
pub fn submenu(anchor: &impl IsA<gtk4::Widget>) -> Option<gtk4::Box> {
    let win = OPEN.with(|o| o.borrow().clone())?;
    let overlay =
        unsafe { win.data::<gtk4::Overlay>("overlay") }.map(|p| unsafe { p.as_ref() }.clone())?;
    let boxes =
        unsafe { win.data::<std::rc::Rc<std::cell::RefCell<Vec<gtk4::Box>>>>("menu_boxes") }
            .map(|p| unsafe { p.as_ref() }.clone())?;

    // Anchor bounds in overlay coordinates — the surface is
    // fullscreen, so widget bounds translate directly.
    let rect = anchor.compute_bounds(&win).map(|b| {
        (
            b.x() as f64,
            b.y() as f64,
            (b.x() + b.width()) as f64,
            (b.y() + b.height()) as f64,
        )
    })?;
    let (ax, ay, ar, _ab) = rect;
    let (sw, _sh) = win
        .surface()
        .and_then(|s| s.display().monitor_at_surface(&s))
        .map(|m| (m.geometry().width() as f64, m.geometry().height() as f64))
        .unwrap_or((1920.0, 1080.0));

    let sub = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sub.add_css_class("menu-box");
    sub.set_hexpand(true);
    sub.set_vexpand(true);
    sub.set_valign(gtk4::Align::Start);
    // Open right of the parent; left when the parent sits past the
    // screen's middle (mirrors the root menu's edge rule).
    if ar > sw / 2.0 {
        sub.set_halign(gtk4::Align::End);
        sub.set_margin_end(((sw - ax).max(0.0) as i32) + 2);
    } else {
        sub.set_halign(gtk4::Align::Start);
        sub.set_margin_start(ar.max(0.0) as i32 + 2);
    }
    sub.set_margin_top(ay.max(0.0) as i32 - 4);
    overlay.add_overlay(&sub);
    boxes.borrow_mut().push(sub.clone());
    Some(sub)
}

/// The widget's full rect plus the bar surface's size — horizontal
/// and vertical — so `shell_at` can position on any edge.
pub fn widget_rect(w: &impl IsA<gtk4::Widget>) -> (f64, f64, f64, f64, f64, f64, i32, i32) {
    let Some(root) = w.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) else {
        return (0.0, 0.0, 0.0, 0.0, 1920.0, 1080.0, 36, 36);
    };
    // The bar surface can sit inboard of the monitor when the bar is
    // narrower than the screen — its left/top layer margins ARE the
    // surface's offset on the output, and menu coordinates are
    // monitor coordinates, so they must be added.
    let ox = root.margin(Edge::Left) as f64;
    let oy = root.margin(Edge::Top) as f64;
    let (l, t, r, b) = w
        .compute_bounds(&root)
        .map(|b| {
            (
                b.x() as f64 + ox,
                b.y() as f64 + oy,
                (b.x() + b.width()) as f64 + ox,
                (b.y() + b.height()) as f64 + oy,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    let (sw, sh) = root
        .surface()
        .and_then(|s| s.display().monitor_at_surface(&s))
        .map(|m| (m.geometry().width() as f64, m.geometry().height() as f64))
        .unwrap_or((root.width() as f64, root.height() as f64));
    (l, t, r, b, sw, sh, root.width(), root.height())
}

/// The applet's horizontal edges, the bar surface's width and the
/// bar window's height — all in bar-surface coordinates. The menu
/// overlay covers the same output, so these values translate 1:1.
pub fn widget_span(w: &impl IsA<gtk4::Widget>) -> (f64, f64, f64, i32) {
    let (l, _t, r, _b, sw, _sh, _bw, bh) = widget_rect(w);
    (l, r, sw, bh)
}

/// A flat vertical list of menu buttons; each runs its action and
/// closes the menu.
pub fn item_list(
    items: Vec<(String, Box<dyn Fn() + 'static>)>,
    _win: &gtk4::Window,
) -> gtk4::Widget {
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    for (label, action) in items {
        let b = gtk4::Button::with_label(&label);
        b.set_has_frame(false);
        b.set_halign(gtk4::Align::Fill);
        b.connect_clicked(move |_| {
            action();
            close_current();
        });
        list.append(&b);
    }
    list.upcast()
}
