// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-livewall — fullscreen watcher
//
// A game or maximised video hides the wallpaper anyway, so
// rendering then is pure waste. This thread opens a second
// Wayland connection (GTK's is busy with the scene), binds
// zwlr_foreign_toplevel_manager_v1 and tracks how many
// toplevels are fullscreen. Any > 0 → paused flag on → the
// scene's tick skips all drawing: zero GPU cost.
// If the compositor lacks the protocol the flag simply never
// turns on — the animation always runs.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Shared watcher state: one fullscreen flag per toplevel,
/// collapsed into the single paused bit the scene reads.
struct State {
    fullscreen: HashMap<wayland_client::backend::ObjectId, bool>,
    paused: Arc<AtomicBool>,
}

impl State {
    /// Recompute the collapsed flag after any per-handle change.
    fn update(&self) {
        let any = self.fullscreen.values().any(|f| *f);
        self.paused.store(any, Ordering::Relaxed);
    }
}

/// Spawn the watcher thread. Never blocks the caller; a missing
/// protocol or dead compositor just leaves `paused` at false.
pub fn start(paused: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(conn) = Connection::connect_to_env() else {
            eprintln!("ragnar-livewall: no wayland connection for watcher");
            return;
        };
        let mut queue = conn.new_event_queue::<State>();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());

        let mut state = State {
            fullscreen: HashMap::new(),
            paused,
        };
        let _ = queue.roundtrip(&mut state);
        while queue.blocking_dispatch(&mut state).is_ok() {}
    });
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        _state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Bind the toplevel manager; version 1 is all we need.
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            if interface == "zwlr_foreign_toplevel_manager_v1" {
                registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(name, 1, qh, ());
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // A new toplevel appeared — track it as non-fullscreen
        // until its first state event says otherwise.
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            state.fullscreen.insert(toplevel.id(), false);
        }
    }

    /// The `toplevel` event creates a child object — teach the
    /// queue which data type its Dispatch impl expects.
    fn event_created_child(
        _opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        qhandle.make_data::<ZwlrForeignToplevelHandleV1, _>(())
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        match event {
            // The state array is raw u32 state flags; flag 3 is
            // fullscreen per the protocol XML.
            Event::State { state: bytes } => {
                let fs = bytes
                    .chunks_exact(4)
                    .any(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 3);
                state.fullscreen.insert(handle.id(), fs);
                state.update();
            }
            Event::Closed => {
                state.fullscreen.remove(&handle.id());
                state.update();
            }
            _ => {}
        }
    }
}
