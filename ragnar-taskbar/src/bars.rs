// Runtime registry of all live bar surfaces plus the app context the
// settings window needs to spawn and manage bars at runtime. Every
// bar's window and shared state live here, indexed by its [bar-N]
// config section, so bars.ini always mirrors the registry.
use crate::bar::{self, State};
use crate::config::BarConfig;
use crate::toplevel::Store;
use crate::tray::Tray;
use crate::workspace::{WsInfo, WsStore};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// One live bar surface plus its mutable config state.
pub struct Entry {
    pub window: gtk4::Window,
    pub state: State,
}

pub type Bars = Rc<RefCell<Vec<Entry>>>;

/// Handles the settings window needs to add/remove/commit bars —
/// captured once at activation so no signature threading is needed.
struct Ctx {
    bars: Bars,
    app: gtk4::Application,
    store: Store,
    tray: Tray,
    ws_store: WsStore,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

/// Spawn every bar described by the config and install the global
/// context. The first bar gets the live tasklist/tray consumers.
pub fn init(
    app: &gtk4::Application,
    store: Store,
    rx: async_channel::Receiver<Vec<crate::toplevel::TopInfo>>,
    tray: Tray,
    tray_rx: async_channel::Receiver<()>,
    ws_store: WsStore,
    ws_rx: async_channel::Receiver<Vec<WsInfo>>,
) {
    let bars: Bars = Rc::new(RefCell::new(Vec::new()));
    let mut first = true;
    for (i, cfg) in crate::config::load().into_iter().enumerate() {
        let (r, tr, wr) = if first {
            (Some(rx.clone()), Some(tray_rx.clone()), Some(ws_rx.clone()))
        } else {
            (None, None, None)
        };
        first = false;
        let mut cfg = cfg;
        cfg.index = i;
        spawn(&bars, app, cfg, &store, r, &tray, tr, &ws_store, wr);
    }
    CTX.with(|c| {
        *c.borrow_mut() = Some(Ctx {
            bars,
            app: app.clone(),
            store,
            tray,
            ws_store,
        })
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    bars: &Bars,
    app: &gtk4::Application,
    cfg: BarConfig,
    store: &Store,
    rx: Option<async_channel::Receiver<Vec<crate::toplevel::TopInfo>>>,
    tray: &Tray,
    tray_rx: Option<async_channel::Receiver<()>>,
    ws_store: &WsStore,
    ws_rx: Option<async_channel::Receiver<Vec<WsInfo>>>,
) {
    let (window, state) = bar::spawn(
        app,
        bars.borrow().len(),
        cfg,
        store,
        rx,
        tray,
        tray_rx,
        ws_store,
        ws_rx,
    );
    bars.borrow_mut().push(Entry { window, state });
}

/// Read access to all live bars.
pub fn bars() -> Bars {
    CTX.with(|c| c.borrow().as_ref().expect("bars live").bars.clone())
}

/// Append a fresh default bar (defaults to the bottom edge, away from
/// any existing one if possible) and persist. Returns its index.
pub fn add() -> usize {
    CTX.with(|c| {
        let b = c.borrow();
        let ctx = b.as_ref().expect("bars live");
        // Put a second bar on the top edge so it never overlaps.
        let mut cfg = BarConfig::default();
        if !ctx.bars.borrow().is_empty() {
            cfg.edge = "top".into();
        }
        spawn(
            &ctx.bars,
            &ctx.app,
            cfg,
            &ctx.store,
            None,
            &ctx.tray,
            None,
            &ctx.ws_store,
            None,
        );
        let idx = ctx.bars.borrow().len() - 1;
        drop(b);
        persist();
        idx
    })
}

/// Remove bar `index` from screen and config.
pub fn remove(index: usize) {
    let b = bars();
    if index < b.borrow().len() {
        let e = b.borrow_mut().remove(index);
        e.window.destroy();
    }
    persist();
}

/// Write `cfg` into bar `index`, persist and rebuild it live.
pub fn commit(index: usize, cfg: BarConfig) {
    let b = bars();
    let e = &b.borrow()[index];
    e.state.borrow_mut().cfg = cfg.clone();
    crate::config::save(index, &cfg);
    bar::update(&e.window, &e.state, cfg);
    bar::rebuild_root(&e.window, &e.state);
}

/// Pull applet `id` out of every bar except `keep` — applets are
/// exclusive: a tasklist or clock can only live on one bar at a time.
/// Bars that lose it are persisted and rebuilt immediately.
pub fn take_applet(id: &str, keep: usize) {
    for (i, e) in bars().borrow().iter().enumerate() {
        if i == keep {
            continue;
        }
        let mut cfg = e.state.borrow().cfg.clone();
        if cfg.layout.iter().any(|x| x == id) {
            cfg.layout.retain(|x| x != id);
            e.state.borrow_mut().cfg = cfg.clone();
            crate::config::save(i, &cfg);
            bar::rebuild_root(&e.window, &e.state);
        }
    }
}

/// Rewrite every bar's config under its live index — INI groups
/// always mirror the registry one-to-one.
fn persist() {
    let count = bars().borrow().len();
    crate::config::drop_stale_bars(count);
    for (i, e) in bars().borrow().iter().enumerate() {
        crate::config::save(i, &e.state.borrow().cfg);
    }
}
