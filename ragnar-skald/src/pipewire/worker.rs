//! PipeWire worker thread: main loop, connection setup, command dispatch.
//!
//! Everything PipeWire-related lives on this dedicated thread. The GTK
//! thread only talks to it through `BackendCommand`s (via the pipewire
//! channel attached to the loop) and receives `BackendEvent`s back via
//! a std channel that the UI drains on a timer.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use anyhow::{Context as _, Result};
use pipewire::channel::{Receiver, Sender};
use pipewire::context::Context;
use pipewire::main_loop::MainLoop;
use tracing::{debug, error, info, warn};

use super::tracker::Tracker;
use super::types::{BackendCommand, BackendEvent};

/// Whether a core-error `res` value means the connection to the
/// PipeWire daemon itself is gone (as opposed to a recoverable
/// per-resource complaint). `res` is a negative errno.
fn is_fatal_errno(res: i32) -> bool {
    // Linux/POSIX values: broken pipe, protocol error, connection
    // reset, not-connected and already-shutdown sockets.
    const EPIPE: i32 = 32;
    const EPROTO: i32 = 71;
    const ECONNRESET: i32 = 104;
    const ENOTCONN: i32 = 107;
    const ESHUTDOWN: i32 = 108;
    matches!(-res, EPIPE | EPROTO | ECONNRESET | ENOTCONN | ESHUTDOWN)
}

/// One-time global PipeWire initialization, safe to call repeatedly.
pub fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(pipewire::init);
}

/// Spawn the worker thread and return the command sender for the GUI.
///
/// Connection problems do not fail `spawn()` itself: the worker pushes a
/// `BackendLost` event so the GUI can show the state instead of exiting.
pub fn spawn(event_tx: mpsc::Sender<BackendEvent>) -> Result<Sender<BackendCommand>> {
    let (cmd_tx, cmd_rx) = pipewire::channel::channel::<BackendCommand>();
    let cmd_tx_clone = cmd_tx.clone();

    std::thread::Builder::new()
        .name("pw-worker".into())
        .spawn(move || {
            if let Err(e) = run(cmd_rx, event_tx.clone()) {
                error!("PipeWire worker exited with error: {e:#}");
                let _ = event_tx.send(BackendEvent::BackendLost(format!("{e:#}")));
            }
        })
        .context("failed to spawn PipeWire worker thread")?;

    Ok(cmd_tx_clone)
}

/// The worker's main body — blocks inside `main_loop.run()` until Quit.
fn run(cmd_rx: Receiver<BackendCommand>, event_tx: mpsc::Sender<BackendEvent>) -> Result<()> {
    init();

    let main_loop = MainLoop::new(None).context("failed to create PipeWire main loop")?;
    let context = Context::new(&main_loop).context("failed to create PipeWire context")?;
    let core = context
        .connect(None)
        .context("failed to connect to PipeWire daemon")?;
    info!("Connected to PipeWire daemon");

    // Detect a lost connection. NOT every core error is fatal: the
    // server also reports recoverable per-resource problems this way
    // (e.g. `unknown resource` — res = -ENOENT — when a rebuild races
    // with an in-flight set_param). Only errnos that mean the socket
    // to the daemon itself died end the session.
    let main_loop_weak = main_loop.downgrade();
    let event_tx_core = event_tx.clone();
    let _core_listener = core
        .add_listener_local()
        .error(move |id, _seq, res, message| {
            warn!("PipeWire core error id={id} res={res}: {message}");
            if id == 0 && is_fatal_errno(res) {
                let _ = event_tx_core.send(BackendEvent::BackendLost(message.to_string()));
                if let Some(ml) = main_loop_weak.upgrade() {
                    ml.quit();
                }
            }
        })
        .register();

    let registry = Rc::new(
        core.get_registry()
            .context("failed to get PipeWire registry")?,
    );

    // All mutable worker state lives in one tracker shared by the
    // registry callbacks and the command receiver. The context is
    // handed in so the tracker can load filter-chain modules.
    let tracker = Rc::new(RefCell::new(Tracker::new(
        event_tx,
        context.clone(),
        core.clone(),
    )));
    // Bound listeners call back into the tracker through a weak self-ref.
    tracker.borrow_mut().set_self(&tracker);

    let tracker_global = Rc::clone(&tracker);
    let tracker_remove = Rc::clone(&tracker);
    let registry_in_global = Rc::clone(&registry);
    let _registry_listener = registry
        .add_listener_local()
        .global(move |obj| {
            tracker_global
                .borrow_mut()
                .add_global(&registry_in_global, obj)
        })
        .global_remove(move |id| tracker_remove.borrow_mut().remove_global(id))
        .register();

    // Commands from the GUI arrive on this attached channel — the pw
    // loop wakes up and dispatches them on the worker thread.
    let tracker_cmd = Rc::clone(&tracker);
    let main_loop_quit = main_loop.downgrade();
    let _cmd_receiver = cmd_rx.attach(main_loop.loop_(), move |cmd| {
        if let BackendCommand::Quit = cmd {
            debug!("Worker received quit command");
            if let Some(ml) = main_loop_quit.upgrade() {
                ml.quit();
            }
            return;
        }
        tracker_cmd.borrow_mut().dispatch_command(cmd);
    });

    main_loop.run();
    // Unload any effect modules still active so their nodes leave the
    // graph cleanly before the context goes away.
    {
        let mut t = tracker.borrow_mut();
        t.cleanup_fx();
        t.cleanup_mic_fx();
    }
    debug!("PipeWire worker main loop stopped");
    Ok(())
}
