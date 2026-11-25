// Copyright (C) Jörg Schröder
// Workspace tracking over the ext-workspace-v1 protocol (staging;
// labwc implements it through wlroots). The compositor reports each
// workspace as a handle with name/state events — this module owns
// the Wayland side on a dedicated thread and streams snapshot bumps
// to the GTK main loop, mirroring the toplevel module's design.
pub mod desktops;
pub mod events;

pub use events::WsShared;

use events::WsState;
use std::cell::RefCell;
use std::rc::Rc;
use wayland_client::Connection;

/// One workspace as the workspace applet sees it.
#[derive(Clone)]
pub struct WsInfo {
    /// Protocol object id — identifies the handle on activation.
    pub id: wayland_client::backend::ObjectId,
    /// Human-readable name the compositor assigned ("1", "2", …).
    pub name: String,
    /// Currently shown workspace.
    pub active: bool,
    /// Workspace requesting attention.
    pub urgent: bool,
}

/// Live store the workspace applet renders from.
#[derive(Clone)]
pub struct WsStore {
    /// Latest snapshot; swapped wholesale on every `done` bump.
    pub wss: Rc<RefCell<Vec<WsInfo>>>,
    /// Wayland-side shared handles + manager, used for activation.
    pub shared: WsShared,
}

impl Default for WsStore {
    fn default() -> Self {
        Self {
            wss: Rc::new(RefCell::new(Vec::new())),
            shared: WsShared::default(),
        }
    }
}

/// Spin up the Wayland thread; returns the store plus the receiver
/// that delivers a fresh snapshot after every `done` batch.
pub fn start() -> (WsStore, async_channel::Receiver<Vec<WsInfo>>) {
    let (notify_tx, notify_rx) = async_channel::unbounded::<Vec<WsInfo>>();
    let store = WsStore::default();
    let shared = store.shared.clone();
    std::thread::spawn(move || {
        let Some(conn) = Connection::connect_to_env().ok() else {
            return;
        };
        *shared.conn.lock().unwrap() = Some(conn.clone());
        let mut queue = conn.new_event_queue::<WsState>();
        let qh = queue.handle();
        let mut state = WsState {
            wss: Vec::new(),
            shared,
            notify: notify_tx,
        };
        let _registry = conn.display().get_registry(&qh, ());
        // One roundtrip binds the manager; the loop then services
        // workspace events until the connection drops.
        let _ = queue.roundtrip(&mut state);
        if state.shared.manager.lock().unwrap().is_none() {
            // Compositor never advertised the global — report an
            // empty snapshot once so the applet renders its honest
            // "unsupported" state instead of waiting forever.
            let _ = state.notify.try_send(Vec::new());
            return;
        }
        while queue.blocking_dispatch(&mut state).is_ok() {}
    });
    (store, notify_rx)
}
