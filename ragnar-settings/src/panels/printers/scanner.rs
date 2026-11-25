// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — printers panel: scanner defaults
// Copyright (C) 2026 LieraOS Project
//
// SANE devices via `scanimage -L`, per-device option ranges via
// `scanimage -d <dev> -A` (exec without shell, like the lp* tools).
// "Save" writes resolution/mode/source into the shared INI file
// ~/.config/ragnar-sap/preferences.ini, group [Scanner:<device-id>] —
// ragnar-sap pre-selects these values in its scan window and updates
// them whenever the user scans with different ones. The file is
// loaded before writing so unrelated groups are preserved.

use gettextrs::gettext;
use gtk4::glib::KeyFile;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

use crate::i18n::msgid;
use super::common;

/// SANE option keys this panel exposes.
const KEYS: &[&str] = &["resolution", "mode", "source"];

/// The scanner card: device dropdown, per-key option dropdowns and
/// the save row. Everything that touches SANE runs off the UI
/// thread — `scanimage -L` initializes the whole backend stack and
/// `-A` really opens the device (~1.5 s on brscan*), so the card
/// shows placeholders and fills in asynchronously.
pub fn card() -> gtk4::ListBox {
    let card = common::card();
    let dev_dd = gtk4::DropDown::from_strings(&[]);
    card.append(&common::row(&gettext("Device"), "", dev_dd.clone().upcast()));

    let opt_dds: Vec<(String, gtk4::DropDown)> = KEYS
        .iter()
        .map(|k| {
            let dd = gtk4::DropDown::from_strings(&[]);
            card.append(&common::row(&gettext(opt_label(k)), "", dd.clone().upcast()));
            (k.to_string(), dd)
        })
        .collect();

    let status = gtk4::Label::new(Some(&gettext("Loading devices…")));
    status.set_xalign(0.0);
    let save = gtk4::Button::with_label(&gettext("Save defaults"));
    save.add_css_class("suggested-action");
    save.set_sensitive(false);
    let btn_row = Box::new(Orientation::Horizontal, 8);
    btn_row.set_valign(gtk4::Align::Center);
    btn_row.append(&save);
    card.append(&common::row("", "", btn_row.upcast()));
    card.append(&common::row("", "", status.clone().upcast()));

    let ids = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    // Generation counter drops stale option fetches when the user
    // switches devices faster than scanimage -A returns.
    let gen = std::rc::Rc::new(std::cell::Cell::new(0u32));

    // Fetch option ranges for the selected device in the background.
    let load_options = std::rc::Rc::new({
        let ids = ids.clone();
        let dds = opt_dds.clone();
        let status = status.clone();
        let gen = gen.clone();
        move |idx: u32| {
            let Some(dev) = ids.borrow().get(idx as usize).cloned() else { return };
            let my_gen = gen.get() + 1;
            gen.set(my_gen);
            status.set_label(&gettext("Reading device options…"));
            for (_, dd) in &dds {
                dd.set_sensitive(false);
            }
            let dds = dds.clone();
            let status = status.clone();
            let gen = gen.clone();
            gtk4::glib::spawn_future_local(async move {
                let d = dev.clone();
                let opts = gtk4::gio::spawn_blocking(move || all_options(&d))
                    .await
                    .unwrap_or_default();
                if gen.get() != my_gen {
                    return; // a newer fetch superseded this one
                }
                for (key, dd) in &dds {
                    let (choices, current) = opts.get(key).cloned().unwrap_or_default();
                    let items: Vec<&str> = choices.iter().map(String::as_str).collect();
                    dd.set_model(Some(&gtk4::StringList::new(&items)));
                    let saved = load_saved(&dev, key);
                    let wanted = saved.as_deref().unwrap_or(&current);
                    if let Some(i) = choices.iter().position(|c| c == wanted) {
                        dd.set_selected(i as u32);
                    }
                    dd.set_sensitive(true);
                }
                status.set_label("");
            });
        }
    });

    // Fetch the device list in the background as well.
    {
        let ids = ids.clone();
        let dev_dd = dev_dd.clone();
        let status = status.clone();
        let save = save.clone();
        let load_options = load_options.clone();
        gtk4::glib::spawn_future_local(async move {
            let devices = gtk4::gio::spawn_blocking(list_devices)
                .await
                .unwrap_or_default();
            if devices.is_empty() {
                status.set_label(&gettext("No scanners found"));
                return;
            }
            let names: Vec<&str> = devices.iter().map(|(_, n)| n.as_str()).collect();
            *ids.borrow_mut() = devices.iter().map(|(id, _)| id.clone()).collect();
            dev_dd.set_model(Some(&gtk4::StringList::new(&names)));
            save.set_sensitive(true);
            load_options(0);
        });
    }
    dev_dd.connect_selected_notify(move |d| load_options(d.selected()));

    // Save the three selections into the shared INI file.
    save.connect_clicked({
        let ids = ids.clone();
        let dds = opt_dds.clone();
        let status = status.clone();
        move |_| {
            let Some(dev) = ids.borrow().get(dev_dd.selected() as usize).cloned() else {
                return;
            };
            let get = |key: &str| -> String {
                dds.iter()
                    .find(|(k, _)| k == key)
                    .and_then(|(_, dd)| {
                        dd.selected_item()
                            .and_downcast::<gtk4::StringObject>()
                            .map(|s| s.string().to_string())
                    })
                    .unwrap_or_default()
            };
            let dpi: i64 = get("resolution").parse().unwrap_or(300);
            let ok = save_defaults(&dev, dpi, &get("mode"), &get("source"));
            status.set_label(&gettext(if ok {
                "Defaults saved — used by Ragnar SAP."
            } else {
                "Could not write the preferences file."
            }));
        }
    });

    card
}

/// One `scanimage -A` run parsed for every key the panel shows.
fn all_options(dev: &str) -> std::collections::HashMap<String, (Vec<String>, String)> {
    let mut map = std::collections::HashMap::new();
    if let Some(out) = run("scanimage", &["-d", dev, "-A"]) {
        for key in KEYS {
            if let Some(v) = parse_option(&out, key) {
                map.insert(key.to_string(), v);
            }
        }
    }
    map
}

/// `scanimage -L` → (device id, display name) pairs.
fn list_devices() -> Vec<(String, String)> {
    run("scanimage", &["-L"])
        .map(|out| {
            out.lines()
                .filter_map(|l| {
                    let l = l.trim();
                    let rest = l
                        .strip_prefix("device `")
                        .or_else(|| l.strip_prefix("device '"))?;
                    // "brother3:bus7;dev1' is a Brother MFC-J410 USB scanner"
                    let (id, desc) = rest.split_once("' is a ")?;
                    Some((id.to_string(), desc.trim().to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `scanimage -d dev -A` → (choices, current) for one option key.
/// Option lines look like `--resolution auto|75|100|300dpi [75]`;
/// mode lists can carry literal brackets ("Gray[Error Diffusion]"),
/// so the current-value marker is the LAST bracket group. `auto`
/// is dropped and a trailing "dpi" unit is normalized away.
/// Extract (choices, current) for `key` from `scanimage -A` output.
fn parse_option(out: &str, key: &str) -> Option<(Vec<String>, String)> {
    for line in out.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("--").and_then(|r| r.strip_prefix(key)) else {
            continue;
        };
        let Some(rest) = rest.strip_prefix(' ') else { continue };
        let Some(open) = rest.rfind('[') else { continue };
        let current = rest[open..].trim_end_matches(']').trim_start_matches('[').trim().to_string();
        let choices = rest[..open]
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty() && *c != "auto")
            .map(|c| c.trim_end_matches("dpi").trim().to_string())
            .collect();
        return Some((choices, current.trim_end_matches("dpi").trim().to_string()));
    }
    None
}

/// Stored default for one option key ("" when unset).
fn load_saved(dev: &str, key: &str) -> Option<String> {
    let kf = KeyFile::new();
    kf.load_from_file(config_path(), gtk4::glib::KeyFileFlags::NONE).ok()?;
    let group = format!("Scanner:{dev}");
    let k = match key {
        "resolution" => "resolution-dpi",
        "mode" => "color-mode",
        "source" => "source",
        _ => return None,
    };
    if key == "resolution" {
        kf.integer(&group, k).ok().map(|v| v.to_string())
    } else {
        kf.string(&group, k).ok().map(|s| s.to_string())
    }
}

/// Write the [Scanner:<dev>] group; existing groups survive because
/// the file is loaded before the keys are set.
fn save_defaults(dev: &str, dpi: i64, mode: &str, source: &str) -> bool {
    let kf = KeyFile::new();
    let path = config_path();
    let _ = kf.load_from_file(&path, gtk4::glib::KeyFileFlags::NONE);
    let group = format!("Scanner:{dev}");
    kf.set_int64(&group, "resolution-dpi", dpi);
    kf.set_string(&group, "color-mode", mode);
    kf.set_string(&group, "source", source);
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    kf.save_to_file(&path).is_ok()
}

/// Same XDG path ragnar-sap resolves for its preferences file.
fn config_path() -> String {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    base.join("ragnar-sap")
        .join("preferences.ini")
        .to_string_lossy()
        .to_string()
}

/// Row label for a SANE option key.
fn opt_label(key: &str) -> &'static str {
    match key {
        "resolution" => msgid("Resolution"),
        "mode" => msgid("Color mode"),
        "source" => msgid("Source"),
        _ => "",
    }
}

/// Run a tool, return trimmed stdout on success.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).output().ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_option;

    const SAMPLE: &str = "\
  --resolution auto|75|100|150|300|600|1200dpi [75]
      Sets the resolution of the scanned image.
  --mode auto|Black & White|Gray[Error Diffusion]|24bit Color [Gray]
  --source auto|FlatBed|Automatic Document Feeder [FlatBed]
";

    #[test]
    fn parses_resolution_choices() {
        let (choices, current) = parse_option(SAMPLE, "resolution").unwrap();
        assert_eq!(current, "75");
        assert_eq!(choices.first().map(String::as_str), Some("75"));
        assert_eq!(choices.last().map(String::as_str), Some("1200"));
    }

    #[test]
    fn bracket_inside_choice_survives() {
        let (choices, _) = parse_option(SAMPLE, "mode").unwrap();
        assert!(choices.iter().any(|c| c.contains("Error Diffusion")));
    }

    #[test]
    fn auto_is_dropped() {
        let (choices, _) = parse_option(SAMPLE, "source").unwrap();
        assert!(!choices.iter().any(|c| c == "auto"));
    }
}
