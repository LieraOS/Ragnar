// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — unified device record,.

use std::collections::HashMap;

/// Device category — enum has exactly these two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
 /// Scan-capable device (SANE backend).
 Scanner,
 /// Print-capable device (CUPS queue).
 Printer,
}

/// One discovered device; metadata mirrors keys
/// (vendor, model, serial, usb_id, usb_class, device_uri,.).
#[derive(Debug, Clone)]
pub struct DeviceInfo {
 /// Backend-specific id (SANE name, CUPS queue, usb:dir).
 pub id: String,
 /// Human-readable name shown in the UI.
 pub display_name: String,
 /// Scanner or Printer.
 pub device_type: DeviceType,
 /// Reachable right now.
 pub online: bool,
 /// Extra attributes collected during discovery.
 pub metadata: HashMap<String, String>,
}
