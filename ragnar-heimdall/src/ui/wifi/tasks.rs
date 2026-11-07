//! Background tasks for the Wi-Fi section: network scans and the
//! periodic read-only association-state sync. Both poll results back
//! onto the main loop through a channel.

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use crate::backend;

use super::{render, Section, POLL_MS};

/// Kick off a scan in a worker thread.
pub(super) fn start_scan(ui: Rc<Section>) {
    ui.scanning.set(true);
    *ui.notice.borrow_mut() = None;
    render(&ui);

    let (tx, rx) = mpsc::channel();
    let iface = ui.iface.clone();
    std::thread::spawn(move || {
        let _ = tx.send(backend::wifi_scan(&iface));
    });

    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(Ok(nets)) => {
                ui.scanning.set(false);
                *ui.nets.borrow_mut() = nets;
                render(&ui);
                gtk4::glib::ControlFlow::Break
            }
            Ok(Err(err)) => {
                tracing::warn!("wifi scan failed: {err:#}");
                ui.scanning.set(false);
                *ui.notice.borrow_mut() =
                    Some(format!("{}: {err:#}", gettextrs::gettext("Scan failed")));
                render(&ui);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}

/// Re-read `Station.ConnectedNetwork` off-thread and fold it into
/// the section state; the view re-renders only when the value
/// actually changed. Read-only — cannot affect the association.
pub(super) fn sync_connected(ui: Rc<Section>) {
    let (tx, rx) = mpsc::channel();
    let iface = ui.iface.clone();
    std::thread::spawn(move || {
        let _ = tx.send(backend::wifi_connected_ssid(&iface));
    });

    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(ssid) => {
                if *ui.connected.borrow() != ssid {
                    *ui.connected.borrow_mut() = ssid;
                    render(&ui);
                }
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
        }
    });
}
