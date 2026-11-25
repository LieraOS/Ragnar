// Copyright (C) Jörg Schröder
//! The desktop surface: a bottom layer-shell window covering the
//! main monitor, carrying the icons on a free-position canvas.

use crate::items::{self, SortBy};
use crate::layout::{self, Arrange, Geo, Positions};
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The desktop window. Owns the icon canvas, positions and menus.
pub struct Surface {
    pub window: gtk4::ApplicationWindow,
    pub(crate) fixed: gtk4::Fixed,
    pub(crate) settings: Option<gio::Settings>,
    /// Connector of the monitor this surface covers (`HDMI-A-1` …);
    /// also the key of its section in the shared layout.
    conn: String,
    /// The monitor this surface covers (for global coordinates).
    mon: gtk4::gdk::Monitor,
    /// The primary output claims every unassigned item.
    primary: bool,
    geo: Cell<Geo>,
    /// All monitors' icon positions: conn → item id → position.
    /// Shared between surfaces — an id in one section is what makes
    /// an icon belong to that monitor.
    shared: Rc<RefCell<std::collections::BTreeMap<String, Positions>>>,
    /// Every surface of the app (self included) for cross-monitor
    /// drags and connected-output checks.
    peers: std::rc::Weak<RefCell<Vec<Rc<Surface>>>>,
    /// Ids of the icons currently shown, in display order.
    pub(crate) shown: RefCell<Vec<String>>,
    /// Item id → its widget on this canvas (reparenting-proof).
    widgets: RefCell<std::collections::HashMap<String, gtk4::Widget>>,
    selected: RefCell<Option<gtk4::Widget>>,
    pending: Cell<bool>,
    /// Ongoing icon drag, when one is in progress.
    pub(crate) drag: RefCell<Option<Drag>>,
    /// Ids of all selected icons (rubber-band selection).
    pub(crate) sel: RefCell<std::collections::HashSet<String>>,
    /// Rubber band: (widget, anchor point) while dragging it out.
    rubber: RefCell<Option<(gtk4::Widget, (f64, f64))>>,
}

/// State of an icon being dragged across the desktop.
pub(crate) struct Drag {
    /// Id of the dragged item.
    pub id: String,
    /// Its widget (moved on the canvas while dragging).
    pub widget: gtk4::Widget,
    /// Position where the drag started.
    pub pos0: (i32, i32),
    /// Pointer position at press, in window coordinates.
    pub press: (f64, f64),
    /// Whether the pointer moved past the drag threshold.
    pub active: bool,
    /// Other selected items pulled along: (id, widget, start pos).
    pub others: Vec<(String, gtk4::Widget, (i32, i32))>,
}

const PANEL_SCHEMA: &str = "org.lieraos.ragnar.panel";
const PRIMARY_KEY: &str = "primary-monitor-list";

/// The panel settings, if the schema is installed (Settings::new
/// would abort the process otherwise).
fn panel_settings() -> Option<gio::Settings> {
    gio::SettingsSchemaSource::default()?.lookup(PANEL_SCHEMA, true)?;
    Some(gio::Settings::new(PANEL_SCHEMA))
}

/// The user's primary monitor: the connector stored by
/// ragnar-display-svc in `primary-monitor-list`. Falls back to the
/// first output when unset or unplugged.
pub(crate) fn main_monitor() -> Option<gtk4::gdk::Monitor> {
    let ms = gtk4::gdk::Display::default()?.monitors();
    let wanted = panel_settings().and_then(|s| s.strv(PRIMARY_KEY).first().map(|c| c.to_string()));
    (0..ms.n_items())
        .filter_map(|i| ms.item(i).and_downcast::<gtk4::gdk::Monitor>())
        .find(|m| wanted.is_some() && m.connector().map(|c| c.to_string()) == wanted)
        .or_else(|| ms.item(0).and_downcast::<gtk4::gdk::Monitor>())
}

impl Surface {
    /// A desktop surface covering `mon`. `primary` marks the main
    /// output; `shared` is every monitor's layout map and `peers`
    /// the live surface list (used for cross-monitor drags).
    pub fn new(
        app: &libadwaita::Application,
        mon: &gtk4::gdk::Monitor,
        primary: bool,
        shared: Rc<RefCell<std::collections::BTreeMap<String, Positions>>>,
        peers: std::rc::Weak<RefCell<Vec<Rc<Surface>>>>,
    ) -> Rc<Self> {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .build();
        // Bottom layer, all edges anchored → fullscreen under
        // everything on this monitor.
        window.init_layer_shell();
        window.set_layer(Layer::Bottom);
        for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(e, true);
        }
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        window.set_monitor(Some(mon));
        window.add_css_class("desktop-surface");

        let conn = mon.connector().map(|c| c.to_string()).unwrap_or_default();
        let g = mon.geometry();
        let fixed = gtk4::Fixed::new();
        // Must always render a pixel: an empty canvas commits no
        // buffer — the surface becomes un-hittable and its last
        // frame lingers as a ghost on monitors without icons.
        fixed.add_css_class("desktop-menu-canvas");
        window.set_child(Some(&fixed));
        let s = Rc::new(Self {
            window: window.clone(),
            fixed,
            settings: crate::prefs::settings(),
            conn,
            mon: mon.clone(),
            primary,
            geo: Cell::new(Geo {
                w: g.width(),
                h: g.height(),
            }),
            shared,
            peers,
            shown: RefCell::new(Vec::new()),
            widgets: RefCell::new(std::collections::HashMap::new()),
            selected: RefCell::new(None),
            pending: Cell::new(false),
            drag: RefCell::new(None),
            sel: RefCell::new(std::collections::HashSet::new()),
            rubber: RefCell::new(None),
        });
        s.install_gestures();
        s.install_actions();
        s.watch();
        s
    }

    /// Bare-desktop clicks: clear selection, right-click opens the
    /// background menu; any press dismisses open dialogs.
    fn install_gestures(self: &Rc<Self>) {
        let bg = gtk4::GestureClick::new();
        bg.set_button(0);
        let s = self.clone();
        bg.connect_released(move |g, _, x, y| {
            // A finished drag ends here — the release can land off
            // the icon, so the icon's own release may never fire.
            if s.end_rubber(x, y) {
                return;
            }
            if s.end_drag(x, y) {
                return;
            }
            if s.over_icon(x, y) {
                return;
            }
            s.clear_selection();
            if g.current_button() == 3 {
                crate::menu::background(&s, &s.window, x, y);
            }
        });
        self.window.add_controller(bg);
        // Pointer motion in window coordinates — the drag math works
        // on a stable frame, unlike GestureDrag whose offsets live
        // in the (moving) widget's coordinate space.
        let motion = gtk4::EventControllerMotion::new();
        let s2 = self.clone();
        motion.connect_motion(move |_, x, y| s2.update_drag(x, y));
        self.window.add_controller(motion);
        let press = gtk4::GestureClick::new();
        press.set_button(1);
        let s3 = self.clone();
        press.connect_pressed(move |_, _, x, y| {
            if !s3.over_icon(x, y) {
                s3.begin_rubber(x, y);
            }
        });
        self.window.add_controller(press);
        // Capture phase, never claims: dialogs close on desktop clicks.
        let dismiss = gtk4::GestureClick::new();
        dismiss.set_button(0);
        dismiss.set_propagation_phase(gtk4::PropagationPhase::Capture);
        dismiss.connect_pressed(|_, _, _, _| crate::dialogs::close_all());
        self.window.add_controller(dismiss);
    }

    /// Settings-backed menu actions (`desk.arrange`, `desk.sort-by`):
    /// radio items in the menus read and write the keys directly.
    /// Menu surfaces on other monitors need the same group.
    pub(crate) fn desk_group(&self) -> Option<gio::SimpleActionGroup> {
        let cfg = self.settings.as_ref()?;
        let group = gio::SimpleActionGroup::new();
        for key in ["arrange", "sort-by"] {
            group.add_action(&cfg.create_action(key));
        }
        Some(group)
    }

    fn install_actions(&self) {
        if let Some(group) = self.desk_group() {
            self.window.insert_action_group("desk", Some(&group));
        }
    }

    /// True when the point (window coordinates) is over an icon.
    pub(crate) fn over_icon(&self, x: f64, y: f64) -> bool {
        let mut w = self.window.pick(x, y, gtk4::PickFlags::DEFAULT);
        while let Some(cur) = w {
            if cur.has_css_class("desktop-icon") {
                return true;
            }
            w = cur.parent();
        }
        false
    }

    pub(crate) fn geo(&self) -> Geo {
        self.geo.get()
    }

    pub(crate) fn arrange(&self) -> Arrange {
        self.settings
            .as_ref()
            .map_or(Arrange::Grid, |s| Arrange::from_key(&s.string("arrange")))
    }

    fn sort(&self) -> SortBy {
        self.settings
            .as_ref()
            .map_or(SortBy::Name, |s| SortBy::from_key(&s.string("sort-by")))
    }

    /// Every live surface (self included).
    pub(crate) fn all_surfaces(&self) -> Vec<Rc<Surface>> {
        self.peers
            .upgrade()
            .map(|l| l.borrow().clone())
            .unwrap_or_default()
    }

    /// Connector ids of every currently attached monitor.
    fn connected(&self) -> std::collections::HashSet<String> {
        let mut c: std::collections::HashSet<String> =
            self.all_surfaces().iter().map(|s| s.conn.clone()).collect();
        c.insert(self.conn.clone());
        c
    }

    /// Sorting re-flows the icons: forget their positions (unless
    /// auto-arranged) so they take grid cells in the new order.
    fn resort(self: &Rc<Self>) {
        if self.arrange() != Arrange::Auto {
            let ids = self.shown.borrow().clone();
            if let Some(p) = self.shared.borrow_mut().get_mut(&self.conn) {
                ids.iter().for_each(|id| {
                    p.remove(id);
                });
            }
        }
        self.refresh();
    }

    /// Debounced rebuild: bursts of file/mount signals collapse into
    /// one refresh, always run from a timeout so no widget is
    /// destroyed while one of its own handlers is still on the stack.
    pub fn refresh(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let s = self.clone();
        gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            s.pending.set(false);
            s.refresh_now();
        });
    }

    /// Re-enumerate items, lay them out and rebuild the canvas.
    /// An item shows on a monitor iff its id lives in that monitor's
    /// layout section — or, on the primary only, when no connected
    /// output has claimed it.
    fn refresh_now(self: &Rc<Self>) {
        self.drag.borrow_mut().take();
        self.selected.replace(None);
        self.sel.borrow_mut().clear();
        while let Some(c) = self.fixed.first_child() {
            self.fixed.remove(&c);
        }
        let list = items::enumerate(self.sort());
        let connected = self.connected();
        let mut mine = self
            .shared
            .borrow()
            .get(&self.conn)
            .cloned()
            .unwrap_or_default();
        let mut claim: Vec<String> = Vec::new();
        let mut visible: Vec<String> = Vec::new();
        for it in &list {
            let id = it.id();
            if mine.contains_key(&id) {
                visible.push(id);
                continue;
            }
            if !self.primary {
                continue;
            }
            let home = self
                .shared
                .borrow()
                .iter()
                .find(|(c, p)| *c != &self.conn && p.contains_key(&id))
                .map(|(c, _)| c.clone());
            match home {
                // Lives on a monitor that is attached right now.
                Some(c) if connected.contains(&c) => {}
                // Unassigned — or assigned to a monitor that is
                // unplugged; the primary claims it.
                _ => {
                    claim.push(id.clone());
                    visible.push(id);
                }
            }
        }
        let spots = layout::place(&visible, &mut mine, self.geo(), self.arrange());
        {
            let mut all = self.shared.borrow_mut();
            for id in &claim {
                for (c, p) in all.iter_mut() {
                    if *c != self.conn && p.remove(id).is_some() {
                        layout::save(c, p);
                    }
                }
            }
            if all.get(&self.conn) != Some(&mine) {
                all.insert(self.conn.clone(), mine.clone());
            }
        }
        layout::save(&self.conn, &mine);
        let mut wmap = self.widgets.borrow_mut();
        wmap.clear();
        for (it, (x, y)) in list
            .iter()
            .filter(|it| visible.contains(&it.id()))
            .zip(&spots)
        {
            let w = self.icon_widget(it);
            self.fixed.put(&w, *x as f64, *y as f64);
            wmap.insert(it.id(), w);
        }
        drop(wmap);
        *self.shown.borrow_mut() = visible;
    }

    /// Pixel spot a drop at `raw` resolves to on this monitor.
    fn drop_spot(&self, raw: (i32, i32)) -> (i32, i32) {
        let others: Vec<(i32, i32)> = {
            let all = self.shared.borrow();
            let mine = all.get(&self.conn);
            self.shown
                .borrow()
                .iter()
                .filter_map(|i| mine.and_then(|m| m.get(i)).copied())
                .collect()
        };
        layout::resolve_drop(self.geo(), self.arrange(), raw, &others)
    }

    /// A drop on this monitor: assign the item to this output's
    /// section (removing it from any other) and persist it.
    pub(crate) fn drop_icon(&self, id: &str, raw: (i32, i32)) -> (i32, i32) {
        let spot = self.drop_spot(raw);
        layout::assign(&mut self.shared.borrow_mut(), &self.conn, id, spot);
        spot
    }

    /// An item was renamed: its remembered position follows it in
    /// whichever monitor's section it lives.
    pub(crate) fn rename_position(&self, old: &str, new: &str) {
        let mut all = self.shared.borrow_mut();
        let home = all
            .iter()
            .find_map(|(c, p)| p.get(old).map(|pos| (c.clone(), *pos)));
        if let Some((c, pos)) = home {
            let p = all.get_mut(&c).expect("section exists");
            p.remove(old);
            p.insert(new.to_string(), pos);
            layout::save(&c, p);
        }
    }

    /// An item is gone: drop its remembered position everywhere.
    pub(crate) fn forget_position(&self, id: &str) {
        let mut all = self.shared.borrow_mut();
        for (c, p) in all.iter_mut() {
            if p.remove(id).is_some() {
                layout::save(c, p);
            }
        }
    }

    /// My section of the shared layout.
    fn my_positions(&self) -> Positions {
        self.shared
            .borrow()
            .get(&self.conn)
            .cloned()
            .unwrap_or_default()
    }

    /// The surface whose monitor contains the global layout point.
    fn surface_at(&self, gx: i32, gy: i32) -> Option<Rc<Surface>> {
        self.all_surfaces().into_iter().find(|s| {
            let g = s.mon.geometry();
            gx >= g.x() && gx < g.x() + g.width() && gy >= g.y() && gy < g.y() + g.height()
        })
    }

    /// Icon press reported by the gesture: remember the possible
    /// drag; it only becomes one past the threshold in update_drag.
    pub(crate) fn begin_drag(&self, id: &str, w: &gtk4::Widget, press: (f64, f64)) {
        let mine = self.my_positions();
        let pos = mine.get(id).copied().unwrap_or_default();
        // Multi-select: every other selected icon moves along.
        let others: Vec<(String, gtk4::Widget, (i32, i32))> = self
            .sel
            .borrow()
            .iter()
            .filter(|i| i.as_str() != id)
            .filter_map(|i| {
                let w = self.icon_of(i)?;
                let p = mine.get(i).copied()?;
                Some((i.clone(), w, p))
            })
            .collect();
        if !self.sel.borrow().contains(id) {
            // Dragging an unselected icon → it becomes the only one.
            self.sel.borrow_mut().clear();
        }
        *self.drag.borrow_mut() = Some(Drag {
            id: id.to_string(),
            widget: w.clone(),
            pos0: pos,
            press,
            active: false,
            others,
        });
    }

    /// The widget showing icon `id` on this canvas.
    fn icon_of(&self, id: &str) -> Option<gtk4::Widget> {
        self.widgets.borrow().get(id).cloned()
    }

    /// Move a dragged widget onto whichever monitor canvas its
    /// global position currently points at, reparenting it across
    /// surfaces when it crosses an output edge. Outside every
    /// monitor it stays clamped to this surface.
    fn move_drag_widget(&self, id: &str, w: &gtk4::Widget, pos0: (i32, i32), dx: i32, dy: i32) {
        let g = self.mon.geometry();
        let (gx, gy) = (g.x() + pos0.0 + dx, g.y() + pos0.1 + dy);
        let Some(t) = self.surface_at(gx, gy) else {
            let local = self.geo().clamp((pos0.0 + dx, pos0.1 + dy));
            self.fixed.move_(w, local.0 as f64, local.1 as f64);
            return;
        };
        let tg = t.mon.geometry();
        let local = t.geo().clamp((gx - tg.x(), gy - tg.y()));
        if w.parent().and_then(|p| p.downcast::<gtk4::Fixed>().ok()) != Some(t.fixed.clone()) {
            if let Some(old) = w.parent().and_then(|p| p.downcast::<gtk4::Fixed>().ok()) {
                old.remove(w);
            }
            // Widget moved canvases: keep only the host's widget map.
            for s in self.all_surfaces() {
                s.widgets.borrow_mut().remove(id);
            }
            t.widgets.borrow_mut().insert(id.to_string(), w.clone());
            t.fixed.put(w, local.0 as f64, local.1 as f64);
        } else {
            t.fixed.move_(w, local.0 as f64, local.1 as f64);
        }
    }

    /// Pointer moved: past the threshold the icon follows 1:1 —
    /// across monitor boundaries too.
    fn update_drag(&self, x: f64, y: f64) {
        if self.rubber.borrow().is_some() {
            self.update_rubber(x, y);
            return;
        }
        let mut guard = self.drag.borrow_mut();
        let Some(d) = guard.as_mut() else { return };
        if self.arrange() == Arrange::Auto {
            *guard = None;
            return;
        }
        let moved = (x - d.press.0).abs() + (y - d.press.1).abs();
        if !d.active && moved < 6.0 {
            return;
        }
        d.active = true;
        let (dx, dy) = ((x - d.press.0) as i32, (y - d.press.1) as i32);
        let (id, w, pos0) = (d.id.clone(), d.widget.clone(), d.pos0);
        let others = d.others.clone();
        drop(guard);
        self.move_drag_widget(&id, &w, pos0, dx, dy);
        for (id, w, pos0) in &others {
            self.move_drag_widget(id, w, *pos0, dx, dy);
        }
    }

    /// Release after a real drag: every icon lands on the monitor it
    /// was dropped on and is re-assigned to that output. Returns
    /// true when a drag was finished (suppress click handling).
    fn end_drag(&self, x: f64, y: f64) -> bool {
        let Some(d) = self.drag.borrow_mut().take() else {
            return false;
        };
        if !d.active {
            return false;
        }
        d.widget.remove_css_class("dragging");
        let (dx, dy) = ((x - d.press.0) as i32, (y - d.press.1) as i32);
        let g = self.mon.geometry();
        let mut ids = vec![(d.id.clone(), d.pos0)];
        ids.extend(d.others.iter().map(|(id, _, p)| (id.clone(), *p)));
        for (id, pos0) in ids {
            let (gx, gy) = (g.x() + pos0.0 + dx, g.y() + pos0.1 + dy);
            match self.surface_at(gx, gy) {
                Some(t) => {
                    let tg = t.mon.geometry();
                    t.drop_icon(&id, (gx - tg.x(), gy - tg.y()));
                }
                None => {
                    self.drop_icon(&id, (pos0.0 + dx, pos0.1 + dy));
                }
            }
        }
        // Rebuild every canvas so icons land on their new monitor
        // and vanish from the old one.
        for s in self.all_surfaces() {
            s.refresh();
        }
        true
    }

    /// Drop all icon highlights.
    pub(crate) fn clear_selection(&self) {
        self.selected.replace(None);
        for id in self.sel.borrow_mut().drain() {
            if let Some(w) = self.icon_of(&id) {
                w.remove_css_class("selected");
            }
        }
    }

    /// Highlight exactly one icon.
    pub(crate) fn select(&self, w: &gtk4::Widget) {
        self.clear_selection();
        w.add_css_class("selected");
        if let Some(id) = self.id_of(w) {
            self.sel.borrow_mut().insert(id);
        }
    }

    /// The item id of an icon widget on this canvas.
    fn id_of(&self, w: &gtk4::Widget) -> Option<String> {
        self.widgets
            .borrow()
            .iter()
            .find(|(_, v)| *v == w)
            .map(|(k, _)| k.clone())
    }

    /// Begin a rubber-band rectangle at (x, y) on empty desktop.
    fn begin_rubber(&self, x: f64, y: f64) {
        self.clear_selection();
        let band = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        band.add_css_class("rubber-band");
        self.fixed.put(&band, x, y);
        *self.rubber.borrow_mut() = Some((band.clone().upcast(), (x, y)));
    }

    /// Resize the rubber band to the pointer.
    fn update_rubber(&self, x: f64, y: f64) {
        let Some((band, (ax, ay))) = self.rubber.borrow().clone() else {
            return;
        };
        let (rx, rw) = (ax.min(x), (x - ax).abs());
        let (ry, rh) = (ay.min(y), (y - ay).abs());
        self.fixed.move_(&band, rx, ry);
        band.set_size_request(rw.max(1.0) as i32, rh.max(1.0) as i32);
    }

    /// Finish the rubber band: select every icon inside the rect.
    /// Returns true when a band was active.
    fn end_rubber(&self, x: f64, y: f64) -> bool {
        let Some((band, (ax, ay))) = self.rubber.borrow_mut().take() else {
            return false;
        };
        self.fixed.remove(&band);
        let (x0, x1) = (ax.min(x), ax.max(x));
        let (y0, y1) = (ay.min(y), ay.max(y));
        if (x1 - x0) + (y1 - y0) < 8.0 {
            return false; // was just a click — clear_selection already ran
        }
        let pos = self.my_positions();
        for id in self.shown.borrow().clone() {
            let Some(&(ix, iy)) = pos.get(&id) else {
                continue;
            };
            let (cx, cy) = (
                ix as f64 + crate::layout::ICON_W as f64 * 0.5,
                iy as f64 + crate::layout::ICON_H as f64 * 0.5,
            );
            let inside = cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1;
            if inside {
                if let Some(w) = self.icon_of(&id) {
                    w.add_css_class("selected");
                    self.sel.borrow_mut().insert(id);
                }
            }
        }
        true
    }

    /// Rebuild icons when ~/Desktop, volumes, the primary monitor
    /// or the arrangement/sort settings change.
    fn watch(self: &Rc<Self>) {
        if let Ok(mon) = items::desktop_dir()
            .monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
        {
            let s = self.clone();
            mon.connect_changed(move |_, _, _, _| s.refresh());
            std::mem::forget(mon); // keep the monitor alive for the app lifetime
        }
        let vm = gio::VolumeMonitor::get();
        for sig in ["mount-added", "mount-removed", "mount-changed"] {
            let s = self.clone();
            vm.connect_local(sig, false, move |_| {
                s.refresh();
                None
            });
        }
        std::mem::forget(vm);
        if let Some(cfg) = &self.settings {
            let s = self.clone();
            cfg.connect_changed(Some("arrange"), move |_, _| s.refresh());
            let s = self.clone();
            cfg.connect_changed(Some("sort-by"), move |_, _| s.resort());
        }
    }
}
