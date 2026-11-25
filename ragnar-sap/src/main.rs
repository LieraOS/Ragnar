// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — entry point.
//
// Without arguments this launches the GTK4 application. Hidden CLI
// flags exist for backend verification on real hardware:
// ragnar-sap --list enumerate SANE devices
// ragnar-sap --printers enumerate CUPS printers
// ragnar-sap --usb enumerate USB scanners
// ragnar-sap --devices full registry() discovery
// ragnar-sap --pipeline-test <in.pnm> <out.ppm> run image pipeline
// ragnar-sap --scan-test <device> <file> scan one page → PNM

mod core;
mod ui;
mod workflows;
mod cups;
mod image;
mod mail;
mod pdf;
mod i18n;
mod sane;
mod usb;

use gtk4::prelude::*;
use gtk4::{gio, Application};

const APP_ID: &str = "org.lieraos.ragnar.Sap";

fn main() {
 i18n::init();
 let args: Vec<String> = std::env::args().collect();
 if args.iter().any(|a| a == "--list") {
 run_list();
 return;
 }
 if let Some(pos) = args.iter().position(|a| a == "--pipeline-test") {
 run_pipeline_test(&args, pos);
 return;
 }
 if args.iter().any(|a| a == "--devices") {
 run_devices();
 return;
 }
 if args.iter().any(|a| a == "--usb") {
 run_usb();
 return;
 }
 if args.iter().any(|a| a == "--printers") {
 run_printers();
 return;
 }
 if let Some(pos) = args.iter().position(|a| a == "--scan-test") {
 run_scan_test(&args, pos);
 return;
 }
 run_app();
}

/// Print every scanner libsane reports, with probed capabilities.
fn run_list() {
 match sane::device::discover() {
 Ok(devs) => {
 if devs.is_empty() {
 println!("{}", i18n::tr("Keine SANE-Geräte gefunden."));
 return;
 }
 for d in &devs {
 println!("{} | {} {} | {}", d.id, d.vendor, d.model, d.device_type);
 if let Ok(s) = sane::session::Session::open(&d.id) {
 println!(" dpi: {:?} modes: {:?} adf: {} duplex: {}",
 s.caps.dpi_list, s.caps.modes, s.caps.has_adf, s.caps.has_duplex
 );
 }
 }
 }
 Err(e) => eprintln!("sane init/discover failed: {e}"),
 }
 sane::init::shutdown();
}

/// Exercise the image pipeline end-to-end: decode a PNM page,
/// auto-crop to content, apply auto colour optimisation and emit
/// a PPM — the same stages ScanWorkflow will chain.
fn run_pipeline_test(args: &[String], pos: usize) {
 let input = args.get(pos + 1).cloned().unwrap_or_default();
 let output = args.get(pos + 2).cloned().unwrap_or_else(|| "/tmp/out.ppm".into());
 let bytes = match std::fs::read(&input) {
 Ok(b) => b,
 Err(e) => {
 eprintln!("read {}: {e}", input);
 return;
 }
 };
 match image::pnm::decode(&bytes) {
 Some(img) => {
 println!("decoded {}x{} ({} bytes)", img.width, img.height, img.data.len());
 let empty = image::empty_page::is_page_empty(&img, image::empty_page::DEFAULT_EMPTY_THRESHOLD);
 println!("empty page: {}", empty);
 let cropped = image::crop::auto_crop(&img);
 println!("cropped -> {}x{}", cropped.width, cropped.height);
 let params = image::color::analyse(&cropped);
 println!("optimize: brightness={} contrast={} saturation={}", params.brightness, params.contrast, params.saturation);
 let opt = image::color::apply(&cropped, &params);
 let rot = image::deskew::detect_rotation(&opt);
 println!("detected rotation: {:?}", rot);
 let rotated = image::crop::rotate(&opt, rot);
 let paper = image::paper::detect_format(rotated.width as i32, rotated.height as i32, 150);
 println!("paper: {} ({}x{}mm)", paper.name, paper.width_mm, paper.height_mm);
 let ppm = image::pnm::encode_ppm(&rotated);
 std::fs::write(&output, &ppm).expect("write output");
 println!("wrote {} ({} bytes)", output, ppm.len());
 }
 None => eprintln!("{}: not a supported PNM (P4/P5/P6)", input),
 }
}

/// Run the full discovery pipeline and dump the registry.
fn run_devices() {
 let registry = core::registry::DeviceRegistry::new();
 core::discovery::discover_all(&registry);
 let all = registry.all();
 if all.is_empty() {
 println!("{}", i18n::tr("Keine Geräte gefunden."));
 return;
 }
 for d in &all {
 println!("{:?} | {} | {} | online: {}", d.device_type, d.id, d.display_name, d.online);
 }
 sane::init::shutdown();
}

/// Print USB scanners found by the sysfs fallback probe.
fn run_usb() {
 let devices = usb::probe::probe(usb::probe::ProbeType::Scanner, "");
 if devices.is_empty() {
 println!("{}", i18n::tr("Keine USB-Scanner gefunden."));
 return;
 }
 for d in &devices {
 println!("{} | {} | {} | class {}", d.id, d.display_name, d.usb_id, d.usb_class);
 }
}

/// Print every physical CUPS printer with PPD capabilities.
fn run_printers() {
 let printers = cups::printer::discover();
 if printers.is_empty() {
 println!("{}", i18n::tr("Keine Drucker gefunden."));
 return;
 }
 for p in &printers {
 println!("{} | {} | {} | default: {}", p.id, p.display_name, p.device_uri, p.is_default);
 if let Some(c) = cups::printer::fetch_capabilities(&p.id) {
 println!(" dpi: {:?} media: {:?} quality: {:?}", c.resolutions_dpi, c.media_sizes, c.qualities);
 }
 }
}

/// Scan one page at 150 dpi gray and store it as a PNM file —
/// the smallest end-to-end proof that the FFI layer works.
fn run_scan_test(args: &[String], pos: usize) {
 let dev = args.get(pos + 1).cloned().unwrap_or_default();
 let out = args.get(pos + 2).cloned().unwrap_or_else(|| "/tmp/scan.pnm".into());
 // Same call sequence : open -> option application ->
 // the scan routine returns ready PNM bytes (header + raster).
 let result = sane::session::Session::open(&dev)
.map_err(|e| e.to_string())
.and_then(|mut s| {
 s.apply_options(&sane::session::ScanOptions {
 resolution_dpi: 150,
 color_mode: "Gray".into(),
 source: String::new(),
 });
 s.set_option("test-picture", "Color pattern").ok();
 s.perform_scan(false).ok_or_else(|| "scan failed".to_string())
 });
 match result {
 Ok(pnm) => {
 std::fs::write(&out, &pnm).expect("write output");
 println!("wrote {} ({} bytes)", out, pnm.len());
 }
 Err(e) => eprintln!("scan failed: {e}"),
 }
 sane::init::shutdown();
}

/// GTK4 shell — dashboard, app actions (prefs/about) and menus.
fn run_app() {
 let app = Application::builder().application_id(APP_ID).build();

 // Application-level actions the dashboard menu wires to.
 let prefs_action = gio::SimpleAction::new("prefs", None);
 let about_action = gio::SimpleAction::new("about", None);
 app.add_action(&prefs_action);
 app.add_action(&about_action);

 app.connect_activate(|app| {
 ui::present_dashboard(app);
 });

 let app_c = app.clone();
 about_action.connect_activate(move |_, _| {
 let app = &app_c;
 let about = gtk4::AboutDialog::builder()
.modal(true)
.program_name("Ragnar SAP")
.version(env!("CARGO_PKG_VERSION"))
.logo_icon_name("scanner")
.copyright("Copyright © 2023-2026 Jörg Schröder")
.comments(i18n::tr("Scannen, Schwärzen und Drucken für Ragnar."))
.website("https://www.lieraos.com")
.website_label("lieraos.com")
.license_type(gtk4::License::Gpl30)
.authors(["Jörg Schröder <info@lieraos..com>"])
.build();
 if let Some(w) = app.active_window() {
 about.set_transient_for(Some(&w));
 }
 about.present();
 });

 let app_p = app.clone();
 prefs_action.connect_activate(move |_, _| {
 let app = &app_p;
 if let Some(w) = app.active_window() {
 ui::prefs_entry(&w);
 }
 });

 app.run_with_args::<&str>(&[]);
}
