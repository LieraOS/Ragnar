// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "System sounds" panel
// Copyright (C) 2026 LieraOS Project
//
// Event-sound configuration: master switch, freedesktop sound theme,
// and a per-event assignment of sound file + volume. Preferences
// live in ~/.config/ragnar-settings/sounds.ini ([sounds], [events],
// [volumes]); the notification sound is additionally written into
// ragnar-raven's settings.ini — the file is the interface, so Raven
// stays fully decoupled. "Open LieraAudio" launches the audio GUI.

use crate::i18n::msgid;
use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::path::{Path, PathBuf};

/// Our own prefs file (panel-level sound configuration).
fn ini_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-settings");
    p.push("sounds.ini");
    p
}

/// Raven's own settings file — we only touch [notifications].
fn raven_ini() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-raven");
    p.push("settings.ini");
    p
}

/// Freedesktop event names offered in the list — only events with a
/// real trigger today (no display-only options): notifications come
/// from Raven, device hot-plug from ragnar-automount, volume diffs
/// from the taskbar's wpctl poll.
const EVENTS: &[(&str, &str)] = &[
    (msgid("Device connected"), "device-added"),
    (msgid("Device removed"), "device-removed"),
    (msgid("Volume changed"), "audio-volume-change"),
];

/// Build the system-sounds panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 24);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 28);
    }
    let cfg = read_ini(&ini_path());
    let cur_theme = get(&cfg, "sounds", "theme").unwrap_or_else(|| "freedesktop".into());
    let files = theme_files(&cur_theme);

    // -- General ----------------------------------------------------------
    let gh = gtk4::Label::new(Some(&gettext("General")));
    gh.add_css_class("heading");
    gh.set_xalign(0.0);
    page.append(&gh);

    let master = gtk4::CheckButton::with_label(&gettext("Event sounds enabled"));
    master.set_active(get(&cfg, "sounds", "enabled").as_deref() != Some("false"));
    master.connect_toggled(|b| {
        write_key(
            &ini_path(),
            "sounds",
            "enabled",
            if b.is_active() { "true" } else { "false" },
        );
    });
    page.append(&master);

    // Sound theme picker.
    let trow = Box::new(Orientation::Horizontal, 12);
    let tl = gtk4::Label::new(Some(&gettext("Sound theme")));
    tl.set_xalign(0.0);
    tl.set_width_chars(22);
    trow.append(&tl);
    let themes = sound_themes();
    let tdd = gtk4::DropDown::from_strings(&themes.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = themes.iter().position(|t| *t == cur_theme) {
        tdd.set_selected(i as u32);
    }
    tdd.set_hexpand(true);
    let themes_c = themes.clone();
    tdd.connect_selected_notify(move |d| {
        if let Some(t) = themes_c.get(d.selected() as usize) {
            write_key(&ini_path(), "sounds", "theme", t);
        }
    });
    trow.append(&tdd);
    page.append(&trow);

    // Deep device/stream config lives in the dedicated audio app.
    let open = gtk4::Button::with_label(&gettext("Open LieraAudio"));
    open.set_halign(gtk4::Align::Start);
    open.connect_clicked(|_| {
        let _ = std::process::Command::new("lieraaudio").spawn();
    });
    page.append(&open);

    page.append(&gtk4::Separator::new(Orientation::Horizontal));

    // -- Notifications (Raven reads its own settings.ini) -----------------
    let nh = gtk4::Label::new(Some(&gettext("Notifications")));
    nh.add_css_class("heading");
    nh.set_xalign(0.0);
    page.append(&nh);
    {
        let rcfg = read_ini(&raven_ini());
        let cur_snd = get(&rcfg, "notifications", "sound").unwrap_or_else(|| "none".into());
        let cur_vol = get(&rcfg, "notifications", "sound_volume")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(80.0);
        let label = gettext("Notification sound");
        let row = event_row(
            EventRowSpec {
                label: &label,
                theme: &cur_theme,
                event: "message-new-instant",
                files: &files,
                cur_snd: &cur_snd,
                cur_vol,
            },
            move |name| write_key(&raven_ini(), "notifications", "sound", name),
            move |v| write_key(&raven_ini(), "notifications", "sound_volume", &v),
        );
        page.append(&row);
    }

    page.append(&gtk4::Separator::new(Orientation::Horizontal));

    // -- Per-event assignment + volume -------------------------------------
    let eh = gtk4::Label::new(Some(&gettext("Event sounds")));
    eh.add_css_class("heading");
    eh.set_xalign(0.0);
    page.append(&eh);
    for (label, event) in EVENTS {
        let cur_snd = get(&cfg, "events", event).unwrap_or_else(|| "default".into());
        let cur_vol = get(&cfg, "volumes", event)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(80.0);
        let ev = event.to_string();
        let ev2 = ev.clone();
        let label = gettext(*label);
        let row = event_row(
            EventRowSpec {
                label: &label,
                theme: &cur_theme,
                event,
                files: &files,
                cur_snd: &cur_snd,
                cur_vol,
            },
            move |name| write_key(&ini_path(), "events", &ev, name),
            move |v| write_key(&ini_path(), "volumes", &ev2, &v),
        );
        page.append(&row);
    }

    page.upcast()
}

/// Everything one event row needs beyond its two persist
/// callbacks — bundled so the builder signature stays readable.
struct EventRowSpec<'a> {
    label: &'a str,
    theme: &'a str,
    event: &'a str,
    files: &'a [String],
    cur_snd: &'a str,
    cur_vol: f64,
}

/// One event row: label, sound dropdown, volume slider — each
/// callback persists its own half into the relevant ini section.
fn event_row<FS, FV>(spec: EventRowSpec, on_sound: FS, on_volume: FV) -> gtk4::Widget
where
    FS: Fn(&str) + 'static,
    FV: Fn(String) + 'static,
{
    let row = Box::new(Orientation::Horizontal, 14);
    let l = gtk4::Label::new(Some(spec.label));
    l.set_xalign(0.0);
    l.set_width_chars(22);
    row.append(&l);

    let mut opts = vec!["default".to_string(), "none".to_string()];
    opts.extend(spec.files.iter().cloned());
    let dd = gtk4::DropDown::from_strings(&opts.iter().map(String::as_str).collect::<Vec<_>>());
    if let Some(i) = opts.iter().position(|o| o == spec.cur_snd) {
        dd.set_selected(i as u32);
    }
    dd.set_hexpand(true);
    let files_c = spec.files.to_vec();
    dd.connect_selected_notify(move |d| {
        let mut v = vec!["default".to_string(), "none".to_string()];
        v.extend(files_c.iter().cloned());
        if let Some(n) = v.get(d.selected() as usize) {
            on_sound(n);
        }
    });
    row.append(&dd);

    let vol = gtk4::Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
    vol.set_width_request(140);
    vol.set_draw_value(true);
    vol.set_value(spec.cur_vol);
    vol.connect_value_changed(move |s| on_volume(format!("{:.0}", s.value())));
    row.append(&vol);

    // Preview: plays the currently selected file at the slider
    // volume; toggles to a stop button while running. Uses gio
    // subprocesses so the main loop never waits on the child.
    let play = gtk4::Button::from_icon_name("media-playback-start-symbolic");
    play.set_has_frame(false);
    play.set_tooltip_text(Some(&gettext("Preview")));
    {
        let playing = std::rc::Rc::new(std::cell::RefCell::new(None::<gtk4::gio::Subprocess>));
        let (dd_c, vol_c, th, ev, fl) = (
            dd.clone(),
            vol.clone(),
            spec.theme.to_string(),
            spec.event.to_string(),
            spec.files.to_vec(),
        );
        play.connect_clicked(move |b| {
            // Running? Stop it and flip back to the play icon.
            if let Some(p) = playing.borrow_mut().take() {
                p.force_exit();
                b.set_icon_name("media-playback-start-symbolic");
                return;
            }
            // Resolve the dropdown value — "default" maps to the
            // event-named file inside the active theme.
            let sel = match dd_c.selected() as usize {
                0 => ev.clone(), // "default" -> event-named file
                1 => return,     // "none" — nothing to preview
                i => match fl.get(i - 2) {
                    Some(n) => n.clone(),
                    None => return,
                },
            };
            let file = theme_file(&th, &sel);
            if !file.exists() {
                return;
            }
            let vol_arg = format!("{:.2}", vol_c.value() / 100.0);
            let Ok(proc_) = gtk4::gio::Subprocess::newv(
                &[
                    std::ffi::OsStr::new("pw-play"),
                    std::ffi::OsStr::new("--volume"),
                    std::ffi::OsStr::new(&vol_arg),
                    file.as_os_str(),
                ],
                gtk4::gio::SubprocessFlags::NONE,
            ) else {
                return;
            };
            *playing.borrow_mut() = Some(proc_.clone());
            b.set_icon_name("media-playback-stop-symbolic");
            // When playback ends (or was stopped), restore the icon.
            let (b2, playing2) = (b.clone(), playing.clone());
            gtk4::glib::spawn_future_local(async move {
                let _ = proc_.wait_future().await;
                b2.set_icon_name("media-playback-start-symbolic");
                *playing2.borrow_mut() = None;
            });
        });
    }
    row.append(&play);
    row.upcast()
}

/// First existing file for `name` in the theme's stereo dir.
fn theme_file(theme: &str, name: &str) -> PathBuf {
    let dir = format!("/usr/share/sounds/{theme}/stereo");
    for ext in ["oga", "ogg", "wav"] {
        let p = PathBuf::from(&dir).join(format!("{name}.{ext}"));
        if p.exists() {
            return p;
        }
    }
    // Unresolved — the spawn simply won't run; callers check exists().
    PathBuf::from(&dir).join(format!("{name}.oga"))
}

/// Sound themes: every dir under /usr/share/sounds with index.theme.
fn sound_themes() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/usr/share/sounds") {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("index.theme").exists() {
                if let Some(n) = p.file_name() {
                    out.push(n.to_string_lossy().to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Sound files (event names) inside a theme's stereo dir.
fn theme_files(theme: &str) -> Vec<String> {
    let dir = format!("/usr/share/sounds/{theme}/stereo");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if matches!(
                p.extension()
                    .map(|x| x.to_string_lossy().to_lowercase())
                    .as_deref(),
                Some("oga") | Some("ogg") | Some("wav")
            ) {
                if let Some(stem) = p.file_stem() {
                    out.push(stem.to_string_lossy().to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Read an ini file into section → key → value.
fn read_ini(
    path: &Path,
) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> {
    let mut map: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> =
        Default::default();
    let mut section = String::new();
    if let Ok(s) = std::fs::read_to_string(path) {
        for line in s.lines() {
            let l = line.trim();
            if l.starts_with('[') && l.ends_with(']') {
                section = l[1..l.len() - 1].to_string();
            } else if let Some((k, v)) = l.split_once('=') {
                map.entry(section.clone())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

fn get(
    m: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    sec: &str,
    key: &str,
) -> Option<String> {
    m.get(sec).and_then(|s| s.get(key)).cloned()
}

/// Set one key inside an ini file — delegates to the shared, tested
/// helper in panels::ini. The previous private copy wrote section
/// headers as `format!("[] {sec}").replace("[] ", "[")` which drops
/// the closing bracket and corrupts the file for every strict INI
/// reader (this is what broke raven's settings.ini).
fn write_key(path: &Path, sec: &str, key: &str, value: &str) {
    let _ = super::ini::write_key(path, sec, key, value);
}
