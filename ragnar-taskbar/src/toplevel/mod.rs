// Foreign-toplevel tracking over the zwlr foreign-toplevel-manager
// protocol (wlroots; labwc supports it). The compositor reports each
// running window as a handle with title/app_id/state events — this
// module owns the Wayland side on a dedicated thread and streams
// snapshot bumps to the GTK main loop.
pub mod events;

pub use events::Shared;

use events::State;
use std::cell::RefCell;
use std::rc::Rc;
use wayland_client::Connection;

/// One tracked window as the task list sees it.
#[derive(Clone)]
pub struct TopInfo {
    /// Protocol object id — identifies the handle on activation.
    pub id: wayland_client::backend::ObjectId,
    pub title: String,
    pub app_id: String,
    pub active: bool,
    /// zwlr minimized state — drives the click toggle and menu labels.
    pub minimized: bool,
    /// zwlr maximized state — drives the maximize/restore menu label.
    pub maximized: bool,
    /// zwlr fullscreen state — bars hide while a toplevel is fullscreen.
    pub fullscreen: bool,
    /// Connector names the toplevel currently covers.
    pub outputs: Vec<String>,
}

/// Live store the tasklist applet renders from.
#[derive(Clone)]
pub struct Store {
    /// Latest snapshot; swapped wholesale on every `done` bump.
    pub tops: Rc<RefCell<Vec<TopInfo>>>,
    /// Wayland-side shared handles, used for activation requests.
    pub shared: Shared,
    /// Applet icon size from the bar config — rebuilt task buttons
    /// read it on every snapshot.
    pub icon_size: Rc<std::cell::Cell<i32>>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            tops: Rc::new(RefCell::new(Vec::new())),
            shared: Shared::default(),
            icon_size: Rc::new(std::cell::Cell::new(24)),
        }
    }
}

/// Spin up the Wayland thread; returns the store plus the receiver
/// that delivers a fresh snapshot after every `done` batch.
pub fn start() -> (Store, async_channel::Receiver<Vec<TopInfo>>) {
    let (notify_tx, notify_rx) = async_channel::unbounded::<Vec<TopInfo>>();
    let store = Store::default();
    let shared = store.shared.clone();
    std::thread::spawn(move || {
        let Some(conn) = connect_wayland() else {
            return;
        };
        *shared.conn.lock().unwrap() = Some(conn.clone());
        let mut queue = conn.new_event_queue::<State>();
        let qh = queue.handle();
        let mut state = State {
            seat: None,
            tops: Vec::new(),
            shared,
            notify: notify_tx,
        };
        let _registry = conn.display().get_registry(&qh, ());
        // One roundtrip binds the manager + seat; the loop then
        // services toplevel events until the connection drops.
        let _ = queue.roundtrip(&mut state);
        while queue.blocking_dispatch(&mut state).is_ok() {}
    });
    (store, notify_rx)
}

/// Own connection to the compositor via $WAYLAND_DISPLAY — a second
/// socket, deliberately NOT GTK's wl_display: two Connections sharing
/// one backend race each other's object tables and would corrupt GTK's
/// own connection (symptom: the bar falls back to a normal window).
fn connect_wayland() -> Option<Connection> {
    Connection::connect_to_env().ok()
}
