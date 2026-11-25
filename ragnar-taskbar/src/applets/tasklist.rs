// Copyright (C) Jörg Schröder
// Task-list applet — one icon button per running toplevel, fed by the
// foreign-toplevel store. Clicking activates (raises) the window and
// the activation is flushed explicitly: queued requests otherwise sit
// in the send buffer while the event queue blocks on compositor
// events. The active window's button gets the `.active` style class.
use crate::toplevel::{Shared, Store, TopInfo};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

/// Per-window buttons kept across snapshots — incremental updates
/// instead of rebuild storms (which flickered on every toplevel
/// event and re-scanned the whole app list per icon).
pub struct Zone {
    pub zone: gtk4::Box,
    /// toplevel id → (button, app_id it was built for)
    pub buttons: std::cell::RefCell<std::collections::HashMap<String, (gtk4::Button, String)>>,
}

/// Snapshot channel the single consumer writes into — the current zone
/// widget is re-registered here whenever the bar layout is rebuilt.
pub type TaskZone = std::rc::Rc<std::cell::RefCell<Option<Zone>>>;

/// Start the ONE snapshot consumer for a bar and return the cell the
/// current zone widget registers itself into. Called once per bar —
/// recreating the layout must never spawn extra consumers (every
/// consumer would steal snapshots from the others).
pub fn start(store: &Store, rx: async_channel::Receiver<Vec<TopInfo>>) -> TaskZone {
    let target: TaskZone = std::rc::Rc::new(std::cell::RefCell::new(None));
    let st = store.clone();
    let tg = target.clone();
    glib::spawn_future_local(async move {
        while let Ok(snap) = rx.recv().await {
            *st.tops.borrow_mut() = snap.clone();
            if let Some(z) = tg.borrow().as_ref() {
                rebuild(z, &st, snap);
            }
            // (unchanged call site — z is now &Zone)
        }
    });
    target
}

/// Create the task-list zone widget for the current layout and
/// register it as the consumer's render target. Repopulates from the
/// cached snapshot so layout changes never flash empty.
pub fn build(store: &Store, target: &TaskZone, vertical: bool) -> gtk4::Widget {
    let zone = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        4,
    );
    zone.set_hexpand(true);
    *target.borrow_mut() = Some(Zone {
        zone: zone.clone(),
        buttons: std::cell::RefCell::new(std::collections::HashMap::new()),
    });
    rebuild(
        target.borrow().as_ref().unwrap(),
        store,
        store.tops.borrow().clone(),
    );
    zone.upcast()
}

/// Incremental update: existing buttons stay (only attributes are
/// updated), vanished windows are removed, new ones appended — no
/// flicker on focus or title changes.
fn rebuild(zone: &Zone, store: &Store, snap: Vec<TopInfo>) {
    let mut seen = std::collections::HashSet::new();
    let mut prev: Option<gtk4::Button> = None;
    for top in &snap {
        let key = format!("{:?}", top.id);
        seen.insert(key.clone());
        let (button, is_new) = {
            let mut buttons = zone.buttons.borrow_mut();
            match buttons.get_mut(&key) {
                Some((b, app)) => {
                    // Update in place: only what can change.
                    b.set_tooltip_text(Some(if top.title.is_empty() {
                        &top.app_id
                    } else {
                        &top.title
                    }));
                    if top.active {
                        b.add_css_class("active");
                    } else {
                        b.remove_css_class("active");
                    }
                    // Rare, but possible: the same handle re-identified
                    // with another app — rebuild the icon then.
                    if *app != top.app_id {
                        let icon = icon_for(&top.app_id, store.icon_size.get());
                        b.set_child(Some(&icon));
                        *app = top.app_id.clone();
                    }
                    (b.clone(), false)
                }
                None => {
                    let b = make_button(store, top);
                    buttons.insert(key.clone(), (b.clone(), top.app_id.clone()));
                    (b, true)
                }
            }
        };
        if is_new {
            zone.zone.append(&button);
        }
        // Visual order = snapshot order: move the button behind the
        // previous one only when it is not already there.
        let ok = match &prev {
            None => button.prev_sibling().is_none(),
            Some(p) => button.prev_sibling().as_ref() == Some(p.upcast_ref()),
        };
        if !ok {
            zone.zone.reorder_child_after(&button, prev.as_ref());
        }
        prev = Some(button);
    }
    // Drop buttons whose toplevel vanished.
    let mut buttons = zone.buttons.borrow_mut();
    let gone: Vec<String> = buttons
        .keys()
        .filter(|k| !seen.contains(*k))
        .cloned()
        .collect();
    for k in gone {
        if let Some((b, _)) = buttons.remove(&k) {
            zone.zone.remove(&b);
        }
    }
}

/// Builds one fresh task button for a newly seen toplevel.
fn make_button(store: &Store, top: &TopInfo) -> gtk4::Button {
    // Icon-first presentation: the app icon carries the button,
    // the window title sits in the tooltip.
    let icon = icon_for(&top.app_id, store.icon_size.get());
    let button = gtk4::Button::new();
    button.set_child(Some(&icon));
    button.set_tooltip_text(Some(if top.title.is_empty() {
        &top.app_id
    } else {
        &top.title
    }));
    button.add_css_class("task-button");
    if top.active {
        button.add_css_class("active");
    }
    // Classic taskbar toggle: an inactive window is raised, the
    // already-active one is minimized on the next click. The active
    // flag is read live — the build-time snapshot goes stale.
    let (shared, id, store2) = (store.shared.clone(), top.id.clone(), store.clone());
    button.connect_clicked(move |_| {
        let active = store2.tops.borrow().iter().any(|t| t.id == id && t.active);
        if active {
            run_action(&shared, &id, Action::Minimize);
        } else {
            activate(&shared, &id);
        }
    });

    // Right-click a task button → window actions, Cinnamon-style.
    let click = gtk4::GestureClick::new();
    click.set_button(3);
    let (shared, id, btn, mn, mx) = (
        store.shared.clone(),
        top.id.clone(),
        button.clone(),
        top.minimized,
        top.maximized,
    );
    click.connect_pressed(move |g, _, _, _| {
        let _ = g;
        task_menu(&btn, &shared, &id, mn, mx);
    });
    button.add_controller(click);

    // Hover → window thumbnail preview above the button.
    let mot = gtk4::EventControllerMotion::new();
    {
        let (btn, shared, id) = (button.clone(), store.shared.clone(), top.id.clone());
        mot.connect_enter(move |_, _, _| {
            let (s, s2, i, i2) = (shared.clone(), shared.clone(), id.clone(), id.clone());
            crate::applets::preview::enter(
                &btn,
                btn.tooltip_text()
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                std::rc::Rc::new(move || activate(&s, &i)),
                std::rc::Rc::new(move || run_action(&s2, &i2, Action::Close)),
            );
        });
    }
    mot.connect_leave(|_| crate::applets::preview::leave());
    button.add_controller(mot);
    button
}

/// Issue the activation request for a toplevel handle.
fn activate(shared: &Shared, id: &wayland_client::backend::ObjectId) {
    let seat = shared.seat.lock().unwrap().clone();
    let handle = shared.handles.lock().unwrap().get(id).cloned();
    if let (Some(seat), Some(handle)) = (seat, handle) {
        handle.activate(&seat);
        if let Some(conn) = shared.conn.lock().unwrap().as_ref() {
            let _ = conn.flush();
        }
    }
}

/// Resolved GIcons per app_id — AppInfo::all() is a full desktop-file
/// scan and must not run on every snapshot/event.
fn cached_icon(app_id: &str) -> Option<gio::Icon> {
    thread_local! {
        static ICONS: std::cell::RefCell<std::collections::HashMap<String, Option<gio::Icon>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    ICONS.with(|c| {
        if let Some(v) = c.borrow().get(app_id) {
            return v.clone();
        }
        let id = app_id.to_lowercase();
        let info = gio::AppInfo::all().into_iter().find(|a| {
            let ai = a.id().map(|i| i.to_lowercase()).unwrap_or_default();
            let ai = ai.strip_suffix(".desktop").unwrap_or(&ai);
            ai == id || ai.rsplit('.').next() == Some(id.as_str())
        });
        let icon = info.and_then(|a| a.icon());
        c.borrow_mut().insert(app_id.to_string(), icon.clone());
        icon
    })
}

/// Resolve the app icon for a toplevel app_id: the id normally matches
/// a .desktop basename; fall back to a generic application icon.
fn icon_for(app_id: &str, size: i32) -> gtk4::Image {
    cached_icon(app_id)
        .map(|i| {
            let img = gtk4::Image::from_gicon(&i);
            img.set_pixel_size(size);
            img
        })
        .unwrap_or_else(|| {
            let img = gtk4::Image::from_icon_name("application-x-executable-symbolic");
            img.set_pixel_size(size);
            img
        })
}

/// Window-action menu at the pointer — a layer-shell menu window, so
/// no input grab can wedge the bar. Labels follow the window state:
/// minimized/maximized windows offer the matching restore instead.
fn task_menu(
    button: &gtk4::Button,
    shared: &Shared,
    id: &wayland_client::backend::ObjectId,
    minimized: bool,
    maximized: bool,
) {
    let (lft, rgt, sw, h) = crate::menu::widget_span(button);
    let mk = |a: Action| -> Box<dyn Fn()> {
        let (s, i) = (shared.clone(), id.clone());
        Box::new(move || run_action(&s, &i, a))
    };
    let items: Vec<(String, Box<dyn Fn()>)> = vec![
        (
            crate::i18n::tr(if minimized {
                "Wiederherstellen"
            } else {
                "Minimieren"
            }),
            mk(if minimized {
                Action::Unminimize
            } else {
                Action::Minimize
            }),
        ),
        (
            crate::i18n::tr(if maximized {
                "Wiederherstellen"
            } else {
                "Maximieren"
            }),
            mk(if maximized {
                Action::Unmaximize
            } else {
                Action::Maximize
            }),
        ),
        (crate::i18n::tr("Schließen"), mk(Action::Close)),
    ];
    let win = crate::menu::shell(lft, rgt, sw, h);
    crate::menu::show(&win, &crate::menu::item_list(items, &win));
}

#[derive(Clone, Copy)]
enum Action {
    Minimize,
    Unminimize,
    Maximize,
    Unmaximize,
    Close,
}

/// Send the chosen window request and flush — same reason activate
/// needed it: queued requests sit in the send buffer otherwise.
fn run_action(shared: &Shared, id: &wayland_client::backend::ObjectId, action: Action) {
    let handle = shared.handles.lock().unwrap().get(id).cloned();
    let Some(handle) = handle else { return };
    match action {
        Action::Minimize => handle.set_minimized(),
        Action::Unminimize => handle.unset_minimized(),
        Action::Maximize => handle.set_maximized(),
        Action::Unmaximize => handle.unset_maximized(),
        Action::Close => handle.close(),
    }
    if let Some(conn) = shared.conn.lock().unwrap().as_ref() {
        let _ = conn.flush();
    }
}

/// Placeholder strip for secondary bars — only the first bar binds the
/// toplevel store for now.
pub fn build_placeholder(vertical: bool) -> gtk4::Widget {
    let zone = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        4,
    );
    zone.set_hexpand(true);
    zone.upcast()
}
