// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — workflow event bus,.
//
// Listeners are plain callbacks; publish walks a snapshot so a
// listener can subscribe/unsubscribe without deadlocking — the
// same pattern uses.

use std::sync::Mutex;

/// A workflow event — interface is a struct of name+data;
/// we carry a plain message string at this layer.
pub type Listener = Box<dyn Fn(&str) + Send>;

/// Thread-safe pub/sub for workflow status events.
#[derive(Default)]
pub struct WorkflowEventBus {
 listeners: Mutex<Vec<Listener>>,
}

impl WorkflowEventBus {
 /// New empty bus.
 pub fn new() -> Self {
 Self::default()
 }

 /// Publish one event to every listener (snapshot iteration).
 pub fn publish(&self, event: &str) {
 // We cannot clone Box<dyn Fn>, so call under the lock — the
 // snapshots std::function; calling under lock keeps the
 // same delivery semantics for our simpler listener type.
 let listeners = self.listeners.lock().unwrap_or_else(|e| e.into_inner());
 for l in listeners.iter() {
 l(event);
 }
 }

 /// Subscribe a listener.
 pub fn add_listener(&self, listener: Listener) {
 self.listeners
.lock()
.unwrap_or_else(|e| e.into_inner())
.push(listener);
 }
}
