// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — user preferences + profiles, of
// PreferencesManager.
//
// saveToFile/loadFromFile were placeholders in — here they
// are real: an INI file at ~/.config/ragnar-sap/preferences.ini
// written via glib::KeyFile, matching the Ragnar convention.

use gtk4::glib::KeyFile;
use std::collections::HashMap;
use std::path::PathBuf;

/// User-facing settings — same defaults as constructor.
#[derive(Debug, Clone)]
pub struct UserPreferences {
 /// "light" | "dark" — kept for equivalence; GTK follows the system.
 pub theme: String,
 /// Base font size.
 pub font_size: i32,
 /// Auto-refresh the device list.
 pub auto_refresh_devices: bool,
 /// Refresh interval in ms.
 pub auto_refresh_interval_ms: i32,
 /// UI language code (de/en/fr/es).
 pub language: String,
 /// Last used scanner id.
 pub last_scanner_device: String,
 /// Last used printer id.
 pub last_printer_device: String,
}

impl Default for UserPreferences {
 fn default() -> Self {
 Self {
 theme: "light".into(),
 font_size: 12,
 auto_refresh_devices: true,
 auto_refresh_interval_ms: 2000,
 language: "system".into(),
 last_scanner_device: String::new(),
 last_printer_device: String::new(),
 }
 }
}

/// Last-used/default scan options for one device — written by
/// ragnar-settings (system defaults) AND by this app after each
/// scan (last-used wins). The scan window pre-selects these values.
#[derive(Debug, Clone, Default)]
pub struct ScannerDefaults {
 /// DPI.
 pub resolution_dpi: i32,
 /// SANE mode string ("Color"/"Gray"/"Lineart") or the literal
 /// option value the settings panel stored.
 pub color_mode: String,
 /// SANE source string ("Flatbed"/"ADF"/"ADF Duplex").
 pub source: String,
}

/// A named scan profile — dpi/mode/source bundle.
#[derive(Debug, Clone, Default)]
pub struct ScanProfile {
 /// Profile name (map key).
 pub name: String,
 /// DPI.
 pub resolution_dpi: i32,
 /// Mode string ("Color"/"Gray"/"Lineart").
 pub color_mode: String,
 /// Source string ("Flatbed"/"ADF"/.).
 pub source: String,
}

/// A named print profile — copies/quality bundle.
#[derive(Debug, Clone, Default)]
pub struct PrintProfile {
 /// Profile name (map key).
 pub name: String,
 /// Number of copies.
 pub copies: i32,
 /// Quality string ("draft"/"normal"/"high").
 pub quality: String,
}

/// Preferences store with real INI persistence.
#[derive(Default)]
pub struct Preferences {
 prefs: UserPreferences,
 scan_profiles: HashMap<String, ScanProfile>,
 print_profiles: HashMap<String, PrintProfile>,
 /// Per-device scan defaults keyed by SANE device id.
 scanner_defaults: HashMap<String, ScannerDefaults>,
}

/// Config file location — XDG-compliant.
pub fn config_path() -> PathBuf {
 let base = std::env::var("XDG_CONFIG_HOME")
.map(std::path::PathBuf::from)
.unwrap_or_else(|_| {
 std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
 });
 base.join("ragnar-sap").join("preferences.ini")
}

impl Preferences {
 /// Load persisted values (or defaults when absent).
 pub fn load() -> Self {
 let mut p = Self::default();
 p.load_from_file(&config_path().to_string_lossy());
 p
 }

 /// Current preferences snapshot.
 pub fn get(&self) -> UserPreferences {
 self.prefs.clone()
 }

 /// Replace all preferences.
 pub fn set(&mut self, prefs: UserPreferences) {
 self.prefs = prefs;
 }

 /// Remember the last scanner.
 pub fn set_last_scanner(&mut self, id: &str) {
 self.prefs.last_scanner_device = id.into();
 }

 /// Remember the last printer.
 pub fn set_last_printer(&mut self, id: &str) {
 self.prefs.last_printer_device = id.into();
 }

 /// Stored scan defaults for a device (ragnar-settings or last use).
 pub fn scanner_defaults(&self, device_id: &str) -> Option<ScannerDefaults> {
 self.scanner_defaults.get(device_id).cloned()
 }

 /// Store scan defaults for a device.
 pub fn set_scanner_defaults(&mut self, device_id: &str, d: ScannerDefaults) {
 self.scanner_defaults.insert(device_id.to_string(), d);
 }

 /// Create a scan profile — false when the name exists (rule).
 pub fn create_scan_profile(&mut self, profile: ScanProfile) -> bool {
 if self.scan_profiles.contains_key(&profile.name) {
 return false;
 }
 self.scan_profiles.insert(profile.name.clone(), profile);
 true
 }

 /// Update a scan profile — false when the name is unknown.
 pub fn update_scan_profile(&mut self, profile: ScanProfile) -> bool {
 if !self.scan_profiles.contains_key(&profile.name) {
 return false;
 }
 self.scan_profiles.insert(profile.name.clone(), profile);
 true
 }

 /// Delete by name — false when absent.
 pub fn delete_scan_profile(&mut self, name: &str) -> bool {
 self.scan_profiles.remove(name).is_some()
 }

 /// All scan profiles.
 pub fn scan_profiles(&self) -> Vec<ScanProfile> {
 self.scan_profiles.values().cloned().collect()
 }

 /// Same CRUD for print profiles.
 pub fn create_print_profile(&mut self, profile: PrintProfile) -> bool {
 if self.print_profiles.contains_key(&profile.name) {
 return false;
 }
 self.print_profiles.insert(profile.name.clone(), profile);
 true
 }

 /// Update a print profile — false when unknown.
 pub fn update_print_profile(&mut self, profile: PrintProfile) -> bool {
 if !self.print_profiles.contains_key(&profile.name) {
 return false;
 }
 self.print_profiles.insert(profile.name.clone(), profile);
 true
 }

 /// Delete by name.
 pub fn delete_print_profile(&mut self, name: &str) -> bool {
 self.print_profiles.remove(name).is_some()
 }

 /// All print profiles.
 pub fn print_profiles(&self) -> Vec<PrintProfile> {
 self.print_profiles.values().cloned().collect()
 }

 /// Persist to an INI file — REAL implementation replacing the
 /// saveToFile placeholder. glib::KeyFile gives the same
 /// format every Ragnar component uses.
 pub fn save_to_file(&self, path: &str) -> bool {
 let kf = KeyFile::new();
 let p = &self.prefs;
 kf.set_string("General", "theme", &p.theme);
 kf.set_integer("General", "font-size", p.font_size);
 kf.set_boolean("General", "auto-refresh-devices", p.auto_refresh_devices);
 kf.set_integer("General", "auto-refresh-interval-ms", p.auto_refresh_interval_ms);
 kf.set_string("General", "language", &p.language);
 kf.set_string("General", "last-scanner", &p.last_scanner_device);
 kf.set_string("General", "last-printer", &p.last_printer_device);
 for (name, sp) in &self.scan_profiles {
 let group = format!("ScanProfile:{}", name);
 kf.set_integer(&group, "resolution-dpi", sp.resolution_dpi);
 kf.set_string(&group, "color-mode", &sp.color_mode);
 kf.set_string(&group, "source", &sp.source);
 }
 for (name, pp) in &self.print_profiles {
 let group = format!("PrintProfile:{}", name);
 kf.set_integer(&group, "copies", pp.copies);
 kf.set_string(&group, "quality", &pp.quality);
 }
 for (dev, d) in &self.scanner_defaults {
 let group = format!("Scanner:{}", dev);
 kf.set_integer(&group, "resolution-dpi", d.resolution_dpi);
 kf.set_string(&group, "color-mode", &d.color_mode);
 kf.set_string(&group, "source", &d.source);
 }
 if let Some(dir) = std::path::Path::new(path).parent() {
 let _ = std::fs::create_dir_all(dir);
 }
 kf.save_to_file(path).is_ok()
 }

 /// Load persisted values — REAL loadFromFile.
 pub fn load_from_file(&mut self, path: &str) -> bool {
 let kf = KeyFile::new();
 if kf.load_from_file(path, gtk4::glib::KeyFileFlags::NONE).is_err() {
 return false;
 }
 let p = &mut self.prefs;
 if let Ok(v) = kf.string("General", "theme") { p.theme = v.to_string(); }
 if let Ok(v) = kf.integer("General", "font-size") { p.font_size = v; }
 if let Ok(v) = kf.boolean("General", "auto-refresh-devices") { p.auto_refresh_devices = v; }
 if let Ok(v) = kf.integer("General", "auto-refresh-interval-ms") { p.auto_refresh_interval_ms = v; }
 if let Ok(v) = kf.string("General", "language") { p.language = v.to_string(); }
 if let Ok(v) = kf.string("General", "last-scanner") { p.last_scanner_device = v.to_string(); }
 if let Ok(v) = kf.string("General", "last-printer") { p.last_printer_device = v.to_string(); }
 for group in kf.groups() {
 let g = group.as_str();
 if let Some(name) = g.strip_prefix("ScanProfile:") {
 self.scan_profiles.insert(name.to_string(), ScanProfile {
 name: name.to_string(),
 resolution_dpi: kf.integer(g, "resolution-dpi").unwrap_or(300),
 color_mode: kf.string(g, "color-mode").map(|s| s.to_string()).unwrap_or_default(),
 source: kf.string(g, "source").map(|s| s.to_string()).unwrap_or_default(),
 });
 } else if let Some(name) = g.strip_prefix("PrintProfile:") {
 self.print_profiles.insert(name.to_string(), PrintProfile {
 name: name.to_string(),
 copies: kf.integer(g, "copies").unwrap_or(1),
 quality: kf.string(g, "quality").map(|s| s.to_string()).unwrap_or_default(),
 });
 } else if let Some(dev) = g.strip_prefix("Scanner:") {
 self.scanner_defaults.insert(dev.to_string(), ScannerDefaults {
 resolution_dpi: kf.integer(g, "resolution-dpi").unwrap_or(300),
 color_mode: kf.string(g, "color-mode").map(|s| s.to_string()).unwrap_or_default(),
 source: kf.string(g, "source").map(|s| s.to_string()).unwrap_or_default(),
 });
 }
 }
 true
 }
}
