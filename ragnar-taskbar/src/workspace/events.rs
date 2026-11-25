// Copyright (C) Jörg Schröder
// Wayland event handling for the workspace thread. Workspace handles
// arrive via the manager, report name/state over time and may be
// removed; every manager `done` pushes the full snapshot to the GTK
// loop. Activation goes through handle.activate() followed by the
// manager's commit — labwc batches workspace requests and only
// applies them on commit.
use crate::workspace::WsInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_registry;
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::ExtWorkspaceGroupHandleV1,
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};

/// ext_workspace_handle_v1 state bits from the protocol XML.
const STATE_ACTIVE: u32 = 1;
const STATE_URGENT: u32 = 2;
const STATE_HIDDEN: u32 = 4;

/// Wayland-side shared state — reachable from GTK for activation.
#[derive(Clone, Default)]
pub struct WsShared {
    /// The thread's connection — GTK-side requests need an explicit
    /// flush or they sit in the send buffer while the event queue is
    /// blocked waiting for compositor events.
    pub conn: Arc<Mutex<Option<Connection>>>,
    /// The bound manager — activate requests only take effect after
    /// its `commit` request, so the GTK side needs the handle.
    pub manager: Arc<Mutex<Option<ExtWorkspaceManagerV1>>>,
    /// Handle lookup by protocol object id.
    pub handles: Arc<Mutex<HashMap<ObjectId, ExtWorkspaceHandleV1>>>,
    /// Arrival order for stable button placement.
    pub order: Arc<Mutex<Vec<ObjectId>>>,
}

/// Per-handle accumulation between `done` events.
pub struct Ws {
    pub id: ObjectId,
    pub name: String,
    pub state: u32,
}

pub struct WsState {
    pub wss: Vec<Ws>,
    pub shared: WsShared,
    /// Snapshot sender to the GTK main loop.
    pub notify: async_channel::Sender<Vec<WsInfo>>,
}

impl WsState {
    /// Push the current workspaces (arrival order, hidden skipped)
    /// to the GTK loop.
    fn publish(&self) {
        let order = self.shared.order.lock().unwrap();
        let snap: Vec<WsInfo> = order
            .iter()
            .filter_map(|id| self.wss.iter().find(|w| w.id == *id))
            .filter(|w| w.state & STATE_HIDDEN == 0)
            .map(|w| WsInfo {
                id: w.id.clone(),
                name: w.name.clone(),
                active: w.state & STATE_ACTIVE != 0,
                urgent: w.state & STATE_URGENT != 0,
            })
            .collect();
        drop(order);
        let _ = self.notify.try_send(snap);
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WsState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<WsState>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
            ..
        } = event
        else {
            return;
        };
        if interface.as_str() == "ext_workspace_manager_v1" {
            let mgr = registry.bind::<ExtWorkspaceManagerV1, _, _>(name, version.min(1), qh, ());
            *state.shared.manager.lock().unwrap() = Some(mgr);
        }
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for WsState {
    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<WsState>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        // XML declaration order: opcode 0 = workspace_group,
        // opcode 1 = workspace — each new_id needs its child type.
        if opcode == 0 {
            qhandle.make_data::<ExtWorkspaceGroupHandleV1, ()>(())
        } else {
            qhandle.make_data::<ExtWorkspaceHandleV1, ()>(())
        }
    }

    fn event(
        state: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WsState>,
    ) {
        use ext_workspace_manager_v1::Event as E;
        match event {
            E::Workspace { workspace } => {
                let id = workspace.id();
                state.wss.push(Ws {
                    id: id.clone(),
                    name: String::new(),
                    state: 0,
                });
                state
                    .shared
                    .handles
                    .lock()
                    .unwrap()
                    .insert(id.clone(), workspace);
                state.shared.order.lock().unwrap().push(id);
            }
            E::Done => state.publish(),
            E::Finished => {
                // Compositor is done with the manager — the applet
                // shows its empty state from here on.
                state.wss.clear();
                state.shared.handles.lock().unwrap().clear();
                state.shared.order.lock().unwrap().clear();
                state.publish();
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for WsState {
    fn event(
        state: &mut Self,
        handle: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WsState>,
    ) {
        use ext_workspace_handle_v1::Event as E;
        let id = handle.id();
        match event {
            E::Name { name } => {
                if let Some(w) = state.wss.iter_mut().find(|w| w.id == id) {
                    w.name = name;
                }
            }
            E::State { state: st } => {
                if let Some(w) = state.wss.iter_mut().find(|w| w.id == id) {
                    w.state = st.into();
                }
            }
            E::Removed => {
                state.wss.retain(|w| w.id != id);
                state.shared.handles.lock().unwrap().remove(&id);
                state.shared.order.lock().unwrap().retain(|o| *o != id);
            }
            _ => {}
        }
    }
}

delegate_noop!(WsState: ignore ExtWorkspaceGroupHandleV1);
