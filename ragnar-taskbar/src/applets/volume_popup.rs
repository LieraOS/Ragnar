// Copyright (C) Jörg Schröder
// Volume popup and context menu for the status applet — the Cinnamon
// sound-applet behaviour on Ragnar:
//   left click  → popover with one fader per playing application
//   right click → menu: open Skald + "enable effects" toggle
//   scroll      → master volume step on the session default output
//
// Data flows through the same channels Skald uses: pw-dump for
// the stream list, wpctl for volume/mute writes, and
// ~/.config/ragnar-skald/settings.json for the effects toggle — the
// ragnar-skald-fx daemon watches that file and applies it.
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// One application row: a shared fader drives every stream the app
/// owns (Firefox can open several sink-inputs at once).
struct AppRow {
    name: String,
    ids: Vec<u32>,
}

/// Collect playback streams from `pw-dump`, grouped per application.
/// Runs through gio::Subprocess like the status polling — a stalled
/// PipeWire must never block the panel's main loop.
fn fetch_streams() -> impl std::future::Future<Output = Vec<AppRow>> {
    async {
        let text = match gio::Subprocess::newv(
            &[std::ffi::OsStr::new("pw-dump")],
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
        ) {
            Ok(p) => p
                .communicate_utf8_future(None)
                .await
                .ok()
                .and_then(|(stdout, _)| stdout)
                .map(|g| g.to_string()),
            Err(_) => None,
        };

        let mut rows: Vec<AppRow> = Vec::new();
        let Some(dump) = text else { return rows };
        let Ok(Value::Array(objs)) = serde_json::from_str::<Value>(&dump) else {
            return rows;
        };
        for o in objs {
            let props = o.pointer("/info/props");
            let Some(p) = props else { continue };
            if p["media.class"].as_str() != Some("Stream/Output/Audio") {
                continue;
            }
            // Our own filter-chain internals expose stream classes —
            // they are plumbing, never user-facing faders.
            let node_name = p["node.name"].as_str().unwrap_or("");
            if node_name.starts_with("skald.") {
                continue;
            }
            let app = p["application.name"]
                .as_str()
                .or_else(|| p["application.process.binary"].as_str())
                .unwrap_or(node_name)
                .to_string();
            let Some(id) = o["id"].as_u64().map(|v| v as u32) else {
                continue;
            };
            match rows.iter_mut().find(|r| r.name == app) {
                Some(r) => r.ids.push(id),
                None => rows.push(AppRow {
                    name: app,
                    ids: vec![id],
                }),
            }
        }
        rows
    }
}

/// `wpctl get-volume <id>` → (fraction, muted). "[MUTED]" rides along
/// in the printed value, so one call answers both.
async fn stream_volume(id: u32) -> Option<(f64, bool)> {
    let arg = id.to_string();
    let out = gio::Subprocess::newv(
        &[
            std::ffi::OsStr::new("wpctl"),
            std::ffi::OsStr::new("get-volume"),
            std::ffi::OsStr::new(&arg),
        ],
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .ok()?
    .communicate_utf8_future(None)
    .await
    .ok()?
    .0?;
    let s = out.to_string();
    let vol = s
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    Some((vol, s.contains("MUTED")))
}

/// Fire-and-forget `wpctl` write — volume or mute on one node.
fn wpctl(args: &[&str]) {
    let argv: Vec<std::ffi::OsString> = std::iter::once(std::ffi::OsString::from("wpctl"))
        .chain(args.iter().map(std::ffi::OsString::from))
        .collect();
    let argv: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_os_str()).collect();
    let _ = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_SILENCE);
}

/// Build one application fader row: name, 0–100 % scale, mute toggle.
fn app_row(row: AppRow) -> gtk4::Widget {
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.set_margin_top(4);
    bx.set_margin_bottom(4);

    let name = gtk4::Label::new(Some(&row.name));
    name.set_width_chars(12);
    name.set_xalign(0.0);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    bx.append(&name);

    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_hexpand(true);
    bx.append(&scale);

    let mute = gtk4::ToggleButton::with_label("M");
    bx.append(&mute);

    // Resolve the current volume of the first stream — grouped rows
    // share one level like the desktop panel does.
    if let Some(&first) = row.ids.first() {
        let scale2 = scale.clone();
        let mute2 = mute.clone();
        glib::spawn_future_local(async move {
            if let Some((vol, muted)) = stream_volume(first).await {
                scale2.set_value(vol * 100.0);
                mute2.set_active(muted);
            }
        });
    }

    let ids = Rc::new(row.ids);
    {
        let ids = Rc::clone(&ids);
        scale.connect_value_changed(move |s| {
            let v = format!("{:.2}", s.value() / 100.0);
            for id in ids.iter() {
                wpctl(&["set-volume", &id.to_string(), &v]);
            }
        });
    }
    mute.connect_toggled(move |b| {
        let on = if b.is_active() { "1" } else { "0" };
        for id in ids.iter() {
            wpctl(&["set-mute", &id.to_string(), on]);
        }
    });
    bx.upcast()
}

/// Left-click popover: live list of playing applications with faders.
pub fn stream_popover(anchor: &impl IsA<gtk4::Widget>) -> gtk4::Popover {
    let pop = gtk4::Popover::new();
    pop.set_parent(anchor);
    pop.set_autohide(true);
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    list.set_margin_start(8);
    list.set_margin_end(8);
    pop.set_child(Some(&list));

    // Rebuild contents on every open so dead streams vanish and new
    // players appear — matches the dynamic list in the main app.
    let list2 = list.clone();
    pop.connect_show(move |_| {
        while let Some(child) = list2.first_child() {
            list2.remove(&child);
        }
        let list3 = list2.clone();
        glib::spawn_future_local(async move {
            let rows = fetch_streams().await;
            if rows.is_empty() {
                let l = gtk4::Label::new(Some(&crate::i18n::tr("Keine Anwendung spielt Audio")));
                list3.append(&l);
                return;
            }
            for r in rows {
                list3.append(&app_row(r));
            }
        });
    });
    pop
}

/// Read `audio_config.fx_enabled` from Skald's settings file.
fn fx_enabled() -> bool {
    dirs_config()
        .and_then(|p| std::fs::read_to_string(p.join("ragnar-skald/settings.json")).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| {
            v.pointer("/audio_config/fx_enabled")
                .map(|b| b == &Value::Bool(true))
        })
        .unwrap_or(false)
}

/// Flip `audio_config.fx_enabled` in settings.json — the fx daemon
/// watches the file and applies the change itself.
fn set_fx_enabled(on: bool) {
    let Some(dir) = dirs_config() else { return };
    let path = dir.join("ragnar-skald/settings.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(mut doc) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if let Some(cfg) = doc.get_mut("audio_config").and_then(|c| c.as_object_mut()) {
        cfg.insert("fx_enabled".to_string(), Value::Bool(on));
        let _ = std::fs::write(
            &path,
            serde_json::to_string_pretty(&doc).unwrap_or_default(),
        );
    }
}

/// XDG config dir without pulling in a `dirs` dependency.
fn dirs_config() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
}

/// Right-click menu: open the full mixer + toggle the saved effects.
pub fn menu_popover(anchor: &impl IsA<gtk4::Widget>) -> gtk4::Popover {
    let pop = gtk4::Popover::new();
    pop.set_parent(anchor);
    pop.set_autohide(true);
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    list.set_margin_start(8);
    list.set_margin_end(8);

    let open = gtk4::Button::with_label(&crate::i18n::tr("Lautstärke öffnen"));
    let fx = gtk4::CheckButton::with_label(&crate::i18n::tr("Effekte aktivieren"));
    list.append(&open);
    list.append(&fx);
    pop.set_child(Some(&list));

    // Reflect the persisted state each time the menu opens.
    let fx2 = fx.clone();
    pop.connect_show(move |_| fx2.set_active(fx_enabled()));

    {
        let pop = pop.clone();
        open.connect_clicked(move |_| {
            let _ = gtk4::glib::spawn_command_line_async("ragnar-skald");
            pop.popdown();
        });
    }
    fx.connect_toggled(move |c| set_fx_enabled(c.is_active()));
    pop
}

/// Scroll step on the session default output: ±5 % per notch.
///
/// The step accumulates on the value we last *wrote*, not on the
/// slowly polling `prev` — otherwise twenty quick notches all re-use
/// the same stale base and scrolling appears to do nothing for
/// seconds. After the write we update `prev` so both the next scroll
/// and the next poll continue from the real level.
///
/// Cap: 100 % normally, 150 % once the level is already boosted —
/// same semantics as the mixer's boost checkbox.
pub fn scroll_step(prev: &Rc<RefCell<Option<(f64, bool)>>>, dy: f64) {
    let (level, muted) = prev.borrow().unwrap_or((1.0, false));
    let max = if level > 1.0 { 1.5 } else { 1.0 };
    let next = (level - dy * 0.05).clamp(0.0, max);
    wpctl(&[
        "set-volume",
        "@DEFAULT_AUDIO_SINK@",
        &format!("{:.2}", next),
    ]);
    *prev.borrow_mut() = Some((next, muted));
}
