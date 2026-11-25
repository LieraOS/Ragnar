// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — device registry(),.
//
// Collects DeviceInfo records from the discovery backends, dedupes
// by (id, type) and normalises metadata defaults. Profiles land
// with ScanProfileManager in a later phase.

use super::device_info::{DeviceInfo, DeviceType};
use std::sync::Mutex;

/// Thread-safe registry() — guards devices_ with a mutex;
/// Mutex<Vec<DeviceInfo>> gives identical semantics.
#[derive(Default)]
pub struct DeviceRegistry {
 devices: Mutex<Vec<DeviceInfo>>,
}

impl DeviceRegistry {
 /// Create an empty registry().
 pub fn new() -> Self {
 Self::default()
 }

 /// Register a device; returns false on a (id, type) duplicate —
 /// dedupe rule.
 pub fn register(&self, mut device: DeviceInfo) -> bool {
 // ensureMetadataDefaults: vendor + model keys always exist.
 device.metadata.entry("vendor".into()).or_default();
 device.metadata.entry("model".into()).or_default();
 let mut devices = self.devices.lock().unwrap_or_else(|e| e.into_inner());
 if devices
.iter()
.any(|d| d.id == device.id && d.device_type == device.device_type)
 {
 return false;
 }
 devices.push(device);
 true
 }

 /// All devices of one type.
 #[allow(dead_code)] // consumed by the dashboard device list
 pub fn list(&self, device_type: DeviceType) -> Vec<DeviceInfo> {
 self.devices
.lock()
.unwrap_or_else(|e| e.into_inner())
.iter()
.filter(|d| d.device_type == device_type)
.cloned()
.collect()
 }

 /// First online device of a type — firstOnline.
 #[allow(dead_code)] // consumed by the workflow defaults
 pub fn first_online(&self, device_type: DeviceType) -> Option<DeviceInfo> {
 self.devices
.lock()
.unwrap_or_else(|e| e.into_inner())
.iter()
.find(|d| d.device_type == device_type && d.online)
.cloned()
 }

 /// Remove every registered device — called before a fresh
 /// discovery pass repopulates the list.
 pub fn clear(&self) {
 self.devices
.lock()
.unwrap_or_else(|e| e.into_inner())
.clear();
 }

 /// Everything currently registered.
 pub fn all(&self) -> Vec<DeviceInfo> {
 self.devices
.lock()
.unwrap_or_else(|e| e.into_inner())
.clone()
 }
}
