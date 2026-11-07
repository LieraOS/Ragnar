// Workspace applet — one labelled button per compositor workspace,
// fed by the ext-workspace store. Clicking activates the workspace:
// the request must be followed by the manager's commit + a
// connection flush or the compositor never applies it (labwc
// batches workspace requests until commit).
use crate::workspace::{WsInfo, WsShared, WsStore};
use gtk4::glib;
use gtk4::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The single snapshot consumer's render target: the current zone
/// widget plus the per-bar display cap. Re-registered on every
/// layout rebuild — the consumer itself is started once per bar.
#[derive(Clone)]
pub struct WsZone {
    zone: Rc<RefCell<Option<gtk4::Box>>>,
    /// 0 = show every workspace the compositor reports.
    max: Rc<Cell<i32>>,
}

/// Start the ONE snapshot consumer for a bar and return the cell the
/// current zone widget registers itself into. Called once per bar —
/// recreating the layout must never spawn extra consumers (every
/// consumer would steal snapshots from the others).
pub fn start(store: &WsStore, rx: async_channel::Receiver<Vec<WsInfo>>) -> WsZone {
    let target = WsZone {
        zone: Rc::new(RefCell::new(None)),
        max: Rc::new(Cell::new(0)),
    };
    let st = store.clone();
    let tg = target.clone();
    glib::spawn_future_local(async move {
        while let Ok(snap) = rx.recv().await {
            *st.wss.borrow_mut() = snap.clone();
            if let Some(z) = tg.zone.borrow().as_ref() {
                rebuild(z, &st, &snap, tg.max.get());
            }
        }
    });
    target
}

/// Create the workspace zone widget for the current layout and
/// register it as the consumer's render target. Repopulates from the
/// cached snapshot so layout changes never flash empty. `max` caps
/// the rendered buttons (0 = show all the compositor reports).
pub fn build(store: &WsStore, target: &WsZone, vertical: bool, max: i32) -> gtk4::Widget {
    let zone = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        2,
    );
    *target.zone.borrow_mut() = Some(zone.clone());
    target.max.set(max);
    rebuild(&zone, store, &store.wss.borrow().clone(), max);
    zone.upcast()
}

/// Re-render on a fresh snapshot, honouring the per-bar cap.
fn rebuild(zone: &gtk4::Box, store: &WsStore, snap: &[WsInfo], max: i32) {
    while let Some(child) = zone.first_child() {
        zone.remove(&child);
    }
    let mut snap = snap.to_vec();
    if max > 0 {
        snap.truncate(max as usize);
    }
    for ws in snap {
        // Compact label: labwc's default names are "Arbeitsfläche N"
        // — far too long for a bar button.
        let short = short_label(&ws.name);
        let button = gtk4::Button::with_label(short);
        button.add_css_class("ws-button");
        if ws.active {
            button.add_css_class("active");
        }
        if ws.urgent {
            button.add_css_class("urgent");
        }
        button.set_tooltip_text(Some(&crate::i18n::trf("Arbeitsfläche {}", &[&ws.name])));
        let (shared, id) = (store.shared.clone(), ws.id.clone());
        button.connect_clicked(move |_| activate(&shared, &id));
        zone.append(&button);
    }
}

/// Compact button label: names ending in a number ("Arbeitsfläche
/// 3", "Workspace 2") collapse to that number; custom names render
/// unchanged. Pure function — unit-tested below.
fn short_label(name: &str) -> &str {
    name.rsplit(' ')
        .next()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(name)
}

/// Ask the compositor to activate a workspace — activate + commit +
/// flush. Without commit labwc queues the request; without flush it
/// stays in our send buffer.
fn activate(shared: &WsShared, id: &wayland_client::backend::ObjectId) {
    let handle = shared.handles.lock().unwrap().get(id).cloned();
    let mgr = shared.manager.lock().unwrap().clone();
    if let (Some(handle), Some(mgr)) = (handle, mgr) {
        handle.activate();
        mgr.commit();
        if let Some(conn) = shared.conn.lock().unwrap().as_ref() {
            let _ = conn.flush();
        }
    }
}

/// Empty strip for secondary bars — only the first bar binds the
/// workspace store for now, mirroring the tasklist.
pub fn build_placeholder(vertical: bool) -> gtk4::Widget {
    gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        2,
    )
    .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_names_collapse() {
        assert_eq!(short_label("Arbeitsfläche 3"), "3");
        assert_eq!(short_label("Workspace 12"), "12");
        assert_eq!(short_label("1"), "1");
    }

    #[test]
    fn custom_names_stay() {
        assert_eq!(short_label("Browser"), "Browser");
        assert_eq!(short_label("IDE Fenster"), "IDE Fenster");
        assert_eq!(short_label("Term2"), "Term2");
    }
}
