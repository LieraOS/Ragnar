// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan + scan-to-print workflows, of
// ScanWorkflow and ScanAndPrintWorkflow.

use super::session::ScanSessionManager;
use std::sync::Arc;

/// Lifecycle state — enum {Idle, Running}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
 /// Not started / finished.
 Idle,
 /// Accepting scan data.
 Running,
}

type StatusCallback = Box<dyn Fn(&str) + Send>;
type DataCallback = Box<dyn Fn(&[u8]) + Send>;

/// Scan workflow — accepts scan payloads into a session and emits
/// the final data when completed. State machine + callback wiring
/// matches single-page semantics.
/// Payload callback shared by scan and print triggers.
pub type PageCallback = Box<dyn Fn(&[u8]) + Send>;

pub struct ScanWorkflow {
 session_manager: Arc<ScanSessionManager>,
 data_callback: Option<DataCallback>,
 status_callback: Option<StatusCallback>,
 session_id: Option<String>,
 state: State,
}

impl ScanWorkflow {
 /// Wire to a session manager(); data callback fires at completion.
 pub fn new(session_manager: Arc<ScanSessionManager>, on_data: DataCallback) -> Self {
 Self {
 session_manager,
 data_callback: Some(on_data),
 status_callback: None,
 session_id: None,
 state: State::Idle,
 }
 }

 /// Status text sink — the UI ticker consumes these.
 pub fn set_status_callback(&mut self, cb: StatusCallback) {
 self.status_callback = Some(cb);
 }

 /// Idle -> Running; no-op when already running.
 pub fn start(&mut self) {
 if self.state == State::Running {
 return;
 }
 self.state = State::Running;
 self.emit("Scan workflow started");
 }

 /// Running -> Idle, aborting any session.
 pub fn stop(&mut self) {
 if self.state == State::Idle {
 return;
 }
 self.finalize(false);
 }

 /// Feed one scanned page/payload into the session.
 pub fn ingest_scan_data(&mut self, data: &[u8]) {
 self.ensure_session();
 match &self.session_id {
 None => self.emit("Failed to create scan session"),
 Some(id) => {
 self.session_manager.update(id, data);
 self.emit("Scan data updated");
 }
 }
 }

 /// Replace the session payload with the redacted version.
 pub fn apply_redaction(&mut self, redacted: &[u8]) {
 self.ensure_session();
 match &self.session_id {
 None => self.emit("Cannot apply redaction without session"),
 Some(id) => {
 self.session_manager.update(id, redacted);
 self.emit("Redaction applied");
 }
 }
 }

 /// Current session id, if any.
 pub fn session_id(&self) -> Option<&str> {
 self.session_id.as_deref()
 }

 /// Load session data, fire the data callback, finalize.
 pub fn complete_and_emit(&mut self) {
 let id = match &self.session_id {
 Some(id) => id.clone(),
 None => {
 self.emit("No session to complete");
 return;
 }
 };
 match self.session_manager.load(&id) {
 Some(data) => {
 if let Some(cb) = &self.data_callback {
 cb(&data);
 }
 self.finalize(true);
 }
 None => self.emit("Failed to load session data for completion"),
 }
 }

 fn ensure_session(&mut self) {
 if self.session_id.is_none() {
 self.session_id = Some(self.session_manager.begin(&[]));
 }
 }

 fn emit(&self, msg: &str) {
 if let Some(cb) = &self.status_callback {
 cb(msg);
 }
 }

 fn finalize(&mut self, success: bool) {
 match self.session_id.take() {
 None => self.state = State::Idle,
 Some(id) => {
 if success {
 self.session_manager.finalize(&id);
 self.emit("Scan session finalized");
 } else {
 self.session_manager.abort(&id);
 self.emit("Scan session aborted");
 }
 self.state = State::Idle;
 }
 }
 }
}

/// Scan+Print orchestration — wraps a ScanWorkflow and adds a
/// print callback on top; verbatim.
pub struct ScanAndPrintWorkflow {
 scan: ScanWorkflow,
 session_manager: Arc<ScanSessionManager>,
 print_callback: Option<PageCallback>,
 status_callback: Option<StatusCallback>,
}

impl ScanAndPrintWorkflow {
 /// The inner scan workflow's data callback becomes the print
 /// path — same wiring as constructor.
 pub fn new(session_manager: Arc<ScanSessionManager>, on_print: DataCallback) -> Self {
 Self {
 scan: ScanWorkflow::new(session_manager.clone(), on_print),
 session_manager,
 print_callback: None,
 status_callback: None,
 }
 }

 /// Status sink for both layers.
 pub fn set_status_callback(&mut self, cb: StatusCallback) {
 // Rust callbacks aren't copyable; we share one status fn by
 // boxing an Rc-free indirection — the UI registers itself on
 // the outer workflow only.
 self.status_callback = Some(cb);
 }

 /// Forward start to ScanWorkflow.
 pub fn start(&mut self) {
 self.scan.start();
 self.emit("Scan & Print workflow ready");
 }

 /// Forward stop.
 pub fn stop(&mut self) {
 self.scan.stop();
 self.emit("Scan & Print workflow stopped");
 }

 /// Forward page data.
 pub fn ingest_scan_data(&mut self, data: &[u8]) {
 self.scan.ingest_scan_data(data);
 }

 /// Forward redaction.
 pub fn apply_redaction(&mut self, redacted: &[u8]) {
 self.scan.apply_redaction(redacted);
 }

 /// Access the inner scan workflow for session ids.
 pub fn scan(&self) -> &ScanWorkflow {
 &self.scan
 }

 /// Mutable access for callback wiring.
 pub fn scan_mut(&mut self) -> &mut ScanWorkflow {
 &mut self.scan
 }

 /// Print trigger — load session payload, fire print callback,
 /// complete ScanWorkflow; ports triggerPrint.
 pub fn trigger_print(&mut self) {
 let id = match self.scan.session_id.as_ref().map(|s| s.to_string()) {
 Some(id) => id,
 None => {
 self.emit("Cannot print without active session");
 return;
 }
 };
 match self.session_manager.load(&id) {
 Some(data) => {
 if let Some(cb) = &self.print_callback {
 cb(&data);
 }
 self.scan.complete_and_emit();
 self.emit("Print job triggered");
 }
 None => self.emit("Failed to load session data for printing"),
 }
 }

 /// Attach the print callback — split from the constructor so the
 /// UI can set it after the workflow exists.
 pub fn set_print_callback(&mut self, cb: PageCallback) {
 self.print_callback = Some(cb);
 }

 fn emit(&self, msg: &str) {
 if let Some(cb) = &self.status_callback {
 cb(msg);
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::core::temp_storage::TempStorage;
 use std::sync::Mutex as StdMutex;
 use std::time::Duration;

 fn manager() -> Arc<ScanSessionManager> {
 let dir = std::env::temp_dir().join(format!("ragnar-sap-wf-{}", std::process::id()));
 Arc::new(ScanSessionManager::new(Arc::new(TempStorage::new(dir)),
 Duration::from_secs(3600),
 ))
 }

 #[test]
 fn scan_workflow_completes_with_payload() {
 let out = Arc::new(StdMutex::new(Vec::<u8>::new()));
 let out2 = out.clone();
 let mgr = manager();
 let mut wf = ScanWorkflow::new(mgr,
 Box::new(move |d| *out2.lock().unwrap() = d.to_vec()),
 );
 wf.start();
 wf.ingest_scan_data(b"page1");
 wf.apply_redaction(b"page1-redacted");
 wf.complete_and_emit();
 assert_eq!(*out.lock().unwrap(), b"page1-redacted".to_vec());
 assert!(wf.session_id.is_none());
 }

 #[test]
 fn scan_and_print_trigger_fires() {
 let printed = Arc::new(StdMutex::new(Vec::<u8>::new()));
 let p2 = printed.clone();
 let mgr = manager();
 let mut wf = ScanAndPrintWorkflow::new(mgr,
 Box::new(move |d| { let _ = d; }),
 );
 wf.set_print_callback(Box::new(move |d| *p2.lock().unwrap() = d.to_vec()));
 wf.start();
 wf.ingest_scan_data(b"scan-payload");
 wf.trigger_print();
 assert_eq!(*printed.lock().unwrap(), b"scan-payload".to_vec());
 }

 #[test]
 fn stop_aborts_session() {
 let mgr = manager();
 let mut wf = ScanWorkflow::new(mgr, Box::new(|_| {}));
 wf.start();
 wf.ingest_scan_data(b"x");
 wf.stop();
 assert!(wf.session_id.is_none());
 }
}
