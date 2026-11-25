// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — background worker helper
//
// SANE scans and CUPS submissions block, so every backend call runs
// on a worker thread and reports back to the GTK main loop. glib 0.20
// dropped `MainContext::channel()`, so results travel through an
// `std::sync::mpsc` channel polled by a short `timeout_add_local`.

use gtk4::glib;

/// Poll cadence for the result channel (ms).
const POLL_MS: u64 = 50;

/// Run `job` on a background thread, then invoke `on_done` with its
/// result on the GTK main loop.
pub fn spawn<T, J, D>(job: J, on_done: D)
where
 T: Send + 'static,
 J: FnOnce() -> T + Send + 'static,
 D: FnOnce(T) + 'static,
{
 let (tx, rx) = std::sync::mpsc::channel::<T>();
 std::thread::spawn(move || {
 let _ = tx.send(job());
 });
 let mut on_done = Some(on_done);
 glib::timeout_add_local(std::time::Duration::from_millis(POLL_MS), move || {
 match rx.try_recv() {
 Ok(value) => {
 if let Some(cb) = on_done.take() {
 cb(value);
 }
 glib::ControlFlow::Break
 }
 Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
 Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
 }
 });
}
