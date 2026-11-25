// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — print workflow,.
//
// Holds an active printer; submitJob hands the payload to a
// callback. When no printer was picked, ensurePrinterSelected
// auto-selects the registry()'s default — same rule.

use crate::core::registry::DeviceRegistry;
use std::sync::Arc;

/// Lifecycle — uses {Idle, Ready}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
 /// Not started.
 Idle,
 /// Ready to accept jobs.
 Ready,
}

type StatusCallback = Box<dyn Fn(&str) + Send>;
type PrintCallback = Box<dyn Fn(&str, &[u8]) + Send>;

/// Print job routing for one selected printer.
pub struct PrintWorkflow {
 devices: Arc<DeviceRegistry>,
 print_callback: Option<PrintCallback>,
 status_callback: Option<StatusCallback>,
 active_printer: Option<String>,
 state: State,
}

impl PrintWorkflow {
 /// Wire to DeviceRegistry + submit callback.
 pub fn new(devices: Arc<DeviceRegistry>, on_print: PrintCallback) -> Self {
 Self {
 devices,
 print_callback: Some(on_print),
 status_callback: None,
 active_printer: None,
 state: State::Idle,
 }
 }

 /// Status text sink.
 pub fn set_status_callback(&mut self, cb: StatusCallback) {
 self.status_callback = Some(cb);
 }

 /// Idle -> Ready, then auto-select a default printer.
 pub fn start(&mut self) {
 if self.state == State::Ready {
 return;
 }
 self.state = State::Ready;
 self.emit("Print workflow ready");
 self.ensure_printer_selected();
 }

 /// Ready -> Idle.
 pub fn stop(&mut self) {
 if self.state == State::Idle {
 return;
 }
 self.state = State::Idle;
 self.emit("Print workflow stopped");
 }

 /// Explicit printer choice — selectPrinter.
 pub fn select_printer(&mut self, device_id: &str) {
 self.active_printer = Some(device_id.to_string());
 self.emit(&format!("Printer selected: {}", device_id));
 }

 /// Current printer id, if one is selected.
 pub fn active_printer(&self) -> Option<&str> {
 self.active_printer.as_deref()
 }

 /// Hand the payload to the print callback — submitJob.
 pub fn submit_job(&mut self, data: &[u8]) {
 if self.state != State::Ready {
 self.emit("Print workflow not ready");
 return;
 }
 self.ensure_printer_selected();
 let printer = match &self.active_printer {
 None => {
 self.emit("No printer available");
 return;
 }
 Some(p) => p.clone(),
 };
 if let Some(cb) = &self.print_callback {
 cb(&printer, data);
 self.emit("Print job submitted");
 } else {
 self.emit("Print callback not configured");
 }
 }

 /// ensurePrinterSelected — pick the registry()'s first online
 /// printer when nothing was chosen yet.
 fn ensure_printer_selected(&mut self) {
 if self.active_printer.is_some() {
 return;
 }
 if let Some(d) = self.devices.first_online(crate::core::device_info::DeviceType::Printer) {
 self.active_printer = Some(d.id.clone());
 self.emit(&format!("Default printer auto-selected: {}", d.id));
 }
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
 use crate::core::device_info::{DeviceInfo, DeviceType};
 use std::collections::HashMap;
 use std::sync::Mutex as StdMutex;

 fn registry_with_printer() -> Arc<DeviceRegistry> {
 let r = DeviceRegistry::new();
 r.register(DeviceInfo {
 id: "cups-0".into(),
 display_name: "Test Printer".into(),
 device_type: DeviceType::Printer,
 online: true,
 metadata: HashMap::new(),
 });
 Arc::new(r)
 }

 #[test]
 fn auto_selects_default_printer() {
 let got = Arc::new(StdMutex::new((String::new(), Vec::<u8>::new())));
 let g2 = got.clone();
 let mut wf = PrintWorkflow::new(registry_with_printer(),
 Box::new(move |dest, data| {
 let mut l = g2.lock().unwrap();
 l.0 = dest.to_string();
 l.1 = data.to_vec();
 }),
 );
 wf.start();
 wf.submit_job(b"job-data");
 let l = got.lock().unwrap();
 assert_eq!(l.0, "cups-0");
 assert_eq!(l.1, b"job-data".to_vec());
 }

 #[test]
 fn explicit_printer_wins() {
 let got = Arc::new(StdMutex::new(String::new()));
 let g2 = got.clone();
 let mut wf = PrintWorkflow::new(registry_with_printer(),
 Box::new(move |dest, _| *g2.lock().unwrap() = dest.to_string()),
 );
 wf.start();
 wf.select_printer("other-queue");
 wf.submit_job(b"x");
 assert_eq!(*got.lock().unwrap(), "other-queue");
 }
}
