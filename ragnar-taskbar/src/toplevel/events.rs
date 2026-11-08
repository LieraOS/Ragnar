// Wayland event handling for the toplevel thread. Handles arrive via
// the manager, report title/app_id/state over time and close on
// destruction; every `done` pushes the full snapshot to the GTK loop.
use crate::toplevel::TopInfo;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_output, wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Wayland-side shared state — reachable from GTK for activation.
#[derive(Clone, Default)]
pub struct Shared {
    /// The thread's connection — GTK-side requests need an explicit
    /// flush or they sit in the send buffer while the event queue is
    /// blocked waiting for compositor events.
    pub conn: Arc<Mutex<Option<Connection>>>,
    pub seat: Arc<Mutex<Option<wl_seat::WlSeat>>>,
    /// Handle lookup by protocol object id — stable across removals.
    pub handles: Arc<Mutex<HashMap<ObjectId, ZwlrForeignToplevelHandleV1>>>,
    /// wl_output object id → connector name ("HDMI-A-1") for
    /// resolving OutputEnter/Leave events.
    pub output_names: Arc<Mutex<HashMap<ObjectId, String>>>,
    /// Arrival order for stable button placement.
    pub order: Arc<Mutex<Vec<ObjectId>>>,
}

/// Per-handle accumulation between `done` events.
pub struct Top {
    pub id: ObjectId,
    pub title: String,
    pub app_id: String,
    pub active: bool,
    pub minimized: bool,
    pub maximized: bool,
    /// zwlr fullscreen state — bars hide while any toplevel is fullscreen.
    pub fullscreen: bool,
    /// Output names (connectors) this toplevel currently sits on —
    /// filled by OutputEnter/Leave so bars hide per-monitor only.
    pub outputs: Vec<String>,
}

pub struct State {
    pub seat: Option<wl_seat::WlSeat>,
    pub tops: Vec<Top>,
    pub shared: Shared,
    /// Snapshot sender to the GTK main loop.
    pub notify: async_channel::Sender<Vec<TopInfo>>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
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
        match interface.as_str() {
            "zwlr_foreign_toplevel_manager_v1" => {
                registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(name, version, qh, ());
            }
            "wl_output" => {
                registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
            }
            "wl_seat" => {
                let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                state.seat = Some(seat.clone());
                *state.shared.seat.lock().unwrap() = Some(seat);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<State>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        make_handle_data(opcode, qhandle)
    }

    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            let id = toplevel.id();
            state.tops.push(Top {
                id: id.clone(),
                title: String::new(),
                app_id: String::new(),
                active: false,
                minimized: false,
                maximized: false,
                fullscreen: false,
                outputs: Vec::new(),
            });
            state
                .shared
                .handles
                .lock()
                .unwrap()
                .insert(id.clone(), toplevel);
            state.shared.order.lock().unwrap().push(id);
        }
    }
}

// The manager's `toplevel` event creates the handle object — the
// specialization tells the dispatcher what child type it is.
fn make_handle_data(
    _opcode: u16,
    qhandle: &QueueHandle<State>,
) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
    qhandle.make_data::<ZwlrForeignToplevelHandleV1, ()>(())
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event as E;
        let id = handle.id();
        match event {
            E::Title { title } => {
                if let Some(t) = state.tops.iter_mut().find(|t| t.id == id) {
                    t.title = title;
                }
            }
            E::AppId { app_id } => {
                if let Some(t) = state.tops.iter_mut().find(|t| t.id == id) {
                    t.app_id = app_id;
                }
            }
            E::State { state: st } => {
                if let Some(t) = state.tops.iter_mut().find(|t| t.id == id) {
                    // zwlr state array values: 0=maximized, 1=minimized,
                    // 2=activated, 3=fullscreen.
                    t.maximized = st.contains(&0);
                    t.minimized = st.contains(&1);
                    t.active = st.contains(&2);
                    t.fullscreen = st.contains(&3);
                }
            }
            E::OutputEnter { output } => {
                if let Some(t) = state.tops.iter_mut().find(|t| t.id == id) {
                    let names = state.shared.output_names.lock().unwrap();
                    if let Some(n) = names.get(&output.id()) {
                        t.outputs.push(n.clone());
                    } else {
                        // name may arrive later — keep the object id
                        t.outputs.push(format!("output-{:?}", output.id()));
                    }
                }
            }
            E::OutputLeave { output } => {
                if let Some(t) = state.tops.iter_mut().find(|t| t.id == id) {
                    let names = state.shared.output_names.lock().unwrap();
                    if let Some(n) = names.get(&output.id()) {
                        t.outputs.retain(|o| o != n);
                    }
                }
            }
            E::Closed => {
                state.tops.retain(|t| t.id != id);
                state.shared.handles.lock().unwrap().remove(&id);
                state.shared.order.lock().unwrap().retain(|o| *o != id);
                // Closed arrives WITHOUT a trailing done — push the
                // snapshot right now or the dead task lingers in the
                // bar until the next unrelated window event.
                let order = state.shared.order.lock().unwrap();
                let snap: Vec<TopInfo> = order
                    .iter()
                    .filter_map(|id| state.tops.iter().find(|t| t.id == *id))
                    .map(|t| TopInfo {
                        id: t.id.clone(),
                        title: t.title.clone(),
                        app_id: t.app_id.clone(),
                        active: t.active,
                        minimized: t.minimized,
                        maximized: t.maximized,
                        fullscreen: t.fullscreen,
                        outputs: t.outputs.clone(),
                    })
                    .collect();
                drop(order);
                let _ = state.notify.try_send(snap);
            }
            E::Done => {
                // Snapshot in arrival order, then wake the GTK loop.
                let order = state.shared.order.lock().unwrap();
                let snap: Vec<TopInfo> = order
                    .iter()
                    .filter_map(|id| state.tops.iter().find(|t| t.id == *id))
                    .map(|t| TopInfo {
                        id: t.id.clone(),
                        title: t.title.clone(),
                        app_id: t.app_id.clone(),
                        active: t.active,
                        minimized: t.minimized,
                        maximized: t.maximized,
                        fullscreen: t.fullscreen,
                        outputs: t.outputs.clone(),
                    })
                    .collect();
                let _ = state.notify.try_send(snap);
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state
                .shared
                .output_names
                .lock()
                .unwrap()
                .insert(proxy.id(), name);
        }
    }
}
