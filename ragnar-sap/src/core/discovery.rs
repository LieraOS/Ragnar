// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — discovery orchestration: feeds SANE scanners, CUPS
// printers and the USB sysfs fallback into the DeviceRegistry,
// mirroring what the three backends report.

use super::device_info::{DeviceInfo, DeviceType};
use std::sync::Mutex;

/// One discovery pass at a time — refresh workers must not run
/// SANE, CUPS and the sysfs probe concurrently.
static DISCOVERY: Mutex<()> = Mutex::new(());
use super::registry::DeviceRegistry;
use crate::{cups, sane, usb};

/// Run all discovery backends and populate the registry.
/// Order mirrors the : SANE scanners first, then CUPS printers,
/// then the USB probe as scanner fallback.
pub fn discover_all(registry: &DeviceRegistry) {
 let _g = DISCOVERY.lock().unwrap_or_else(|e| e.into_inner());
 for d in sane::device::discover().unwrap_or_default() {
 let mut metadata = std::collections::HashMap::new();
 metadata.insert("vendor".into(), d.vendor.clone());
 metadata.insert("model".into(), d.model.clone());
 metadata.insert("device_type".into(), d.device_type.clone());
 metadata.insert("backend".into(), "sane".into());
 registry.register(DeviceInfo {
 id: d.id.clone(),
 display_name: if d.model.is_empty() { d.id } else { d.model },
 device_type: DeviceType::Scanner,
 online: true,
 metadata,
 });
 }
 let printers = cups::printer::discover();
 for p in &printers {
 let mut metadata = std::collections::HashMap::new();
 metadata.insert("is_default".into(), p.is_default.to_string());
 metadata.insert("location".into(), p.location.clone());
 metadata.insert("make_and_model".into(), p.display_name.clone());
 metadata.insert("device_uri".into(), p.device_uri.clone());
 metadata.insert("backend".into(), "cups".into());
 registry.register(DeviceInfo {
 id: p.id.clone(),
 display_name: p.display_name.clone(),
 device_type: DeviceType::Printer,
 online: true,
 metadata,
 });
 }
 // USB fallback — runs the sysfs probe when CUPS reports
 // no queues; for scanners it complements SANE's backend list.
 if printers.is_empty() {
 for u in usb::probe::probe(usb::probe::ProbeType::Other, "printer") {
 let mut metadata = std::collections::HashMap::new();
 metadata.insert("vendor".into(), u.vendor);
 metadata.insert("model".into(), u.model);
 metadata.insert("serial".into(), u.serial);
 metadata.insert("usb_id".into(), u.usb_id);
 metadata.insert("usb_class".into(), u.usb_class);
 metadata.insert("backend".into(), "usb".into());
 registry.register(DeviceInfo {
 id: u.id,
 display_name: u.display_name,
 device_type: DeviceType::Printer,
 online: true,
 metadata,
 });
 }
 }
 for u in usb::probe::probe(usb::probe::ProbeType::Scanner, "") {
 let mut metadata = std::collections::HashMap::new();
 metadata.insert("vendor".into(), u.vendor);
 metadata.insert("model".into(), u.model);
 metadata.insert("serial".into(), u.serial);
 metadata.insert("usb_id".into(), u.usb_id);
 metadata.insert("usb_class".into(), u.usb_class);
 metadata.insert("backend".into(), "usb".into());
 registry.register(DeviceInfo {
 id: u.id,
 display_name: u.display_name,
 device_type: DeviceType::Scanner,
 online: true,
 metadata,
 });
 }
}
