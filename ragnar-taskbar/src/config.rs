// Copyright (C) Jörg Schröder
// Per-bar configuration, stored as an INI keyfile so the taskbar runs
// before any GSettings schema is installed. Every [bar-N] section
// describes one layer-shell surface; an empty file falls back to a
// single classic bottom bar.
use gtk4::glib;

const CFG_DIR: &str = "ragnar-taskbar";
const CFG_FILE: &str = "bars.ini";

/// Horizontal placement when the bar does not span the full monitor.
#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Item ids allowed inside a bar layout — applet builders plus the
/// expanding "spacer" that separates groups visually.
pub const KNOWN_APPLETS: &[&str] = &["menu", "tasklist", "workspace", "tray", "status", "clock"];

/// Classic arrangement: menu left, tasks centre, tray+clock right.
pub const LAYOUT_DEFAULT: &[&str] = &[
    "menu", "spacer", "tasklist", "spacer", "tray", "status", "notify", "clock",
];

/// Layout description for one bar surface.
#[derive(Clone)]
pub struct BarConfig {
    /// "bottom" or "top" — which screen edge the bar docks to.
    pub edge: String,
    /// Bar thickness in pixels.
    pub height: i32,
    /// Bar length as percent of the monitor width (30–100).
    pub width_percent: i32,
    /// Placement of a non-full-width bar.
    pub align: Align,
    /// Ordered left-to-right item list — applet ids and "spacer"
    /// entries. The bar renders exactly this sequence.
    pub layout: Vec<String>,
    /// Applet icon size in pixels (status icons, task buttons, menu).
    pub icon_size: i32,
    /// Background opacity in percent (10–100).
    pub opacity: i32,
    /// Custom bar colour as "#rrggbb" — empty = theme default.
    pub color: String,
    /// Output connector ("HDMI-A-1", "eDP-1", …) this bar pins to.
    /// Empty = the first monitor — Wayland has no "primary" concept,
    /// so the bar must be bound to an output explicitly.
    pub monitor: String,
    /// Cap on workspace buttons the workspace applet renders;
    /// 0 = show every workspace the compositor reports.
    pub workspace_max: i32,
    /// Position of the [bar-N] section — picks monitor N when
    /// `monitor` is empty, so each bar lands on a different output
    /// instead of all piling onto the first one.
    pub index: usize,
    /// Animated gradient background — off by default; when on, the
    /// bar paints a flowing colour gradient instead of a solid fill.
    pub anim: bool,
    /// Gradient colours as "#rrggbb" — at least two for a gradient.
    pub anim_colors: Vec<String>,
    /// Flow direction: "ltr" | "rtl" | "down" | "up" | "spin".
    pub anim_dir: String,
    /// Flow speed 1–10 — a full colour cycle takes `13 - speed`
    /// seconds, so higher means faster.
    pub anim_speed: i32,
    /// Repaint rate preset: 15 | 30 | 60 fps.
    pub anim_fps: i32,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            edge: "bottom".into(),
            height: 36,
            width_percent: 100,
            align: Align::Center,
            layout: LAYOUT_DEFAULT.iter().map(|s| s.to_string()).collect(),
            icon_size: 24,
            opacity: 100,
            color: String::new(),
            monitor: String::new(),
            workspace_max: 0,
            index: 0,
            anim: false,
            anim_colors: vec!["#3584e4".into(), "#9141ac".into(), "#e01b24".into()],
            anim_dir: "ltr".into(),
            anim_speed: 4,
            anim_fps: 30,
        }
    }
}

/// Remove `[bar-N]` groups with N >= `count` — stale sections from a
/// deleted bar must not resurrect it on the next launch.
pub fn drop_stale_bars(count: usize) {
    let kf = glib::KeyFile::new();
    if kf.load_from_file(path(), glib::KeyFileFlags::NONE).is_err() {
        return;
    }
    for g in kf.groups() {
        if let Some(n) = g.strip_prefix("bar-").and_then(|s| s.parse::<usize>().ok()) {
            if n >= count {
                let _ = kf.remove_group(&g);
            }
        }
    }
    let _ = kf.save_to_file(path());
}

fn path() -> std::path::PathBuf {
    cfg_dir().join(CFG_FILE)
}

/// The app's config directory — shared by every config file
/// (bars.ini, menu.ini, …).
pub fn cfg_dir() -> std::path::PathBuf {
    glib::user_config_dir().join(CFG_DIR)
}

/// Load all bar sections; no file / no sections -> one default bar.
pub fn load() -> Vec<BarConfig> {
    let kf = glib::KeyFile::new();
    if kf.load_from_file(path(), glib::KeyFileFlags::NONE).is_err() {
        return vec![BarConfig::default()];
    }
    let mut groups: Vec<String> = kf.groups().iter().map(|g| g.to_string()).collect();
    if groups.is_empty() {
        return vec![BarConfig::default()];
    }
    // Deterministic load order — groups come back unordered, so
    // "bar-10" would sort before "bar-2" lexically; parse the index.
    groups.sort_by_key(|g| {
        g.strip_prefix("bar-")
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    });
    groups
        .iter()
        .filter_map(|g| {
            let edge = kf.string(g.as_str(), "edge").ok()?.to_string();
            let height = kf.int64(g.as_str(), "height").unwrap_or(36) as i32;
            let width_percent = kf.int64(g.as_str(), "width-percent").unwrap_or(100) as i32;
            let layout = kf
                .string(g, "layout")
                .map(|raw| {
                    let v: Vec<String> = raw
                        .split(';')
                        .filter(|e| {
                            // Keep every entry the bar can render —
                            // spacers, known applets, sensor-* and
                            // user-defined cmd: entries. Anything else
                            // silently dropped the bar's content on
                            // load.
                            *e == "spacer"
                                || e.starts_with("spacer:")
                                || e.starts_with("sensor-")
                                || e.starts_with("cmd:")
                                || KNOWN_APPLETS.contains(e)
                        })
                        .map(str::to_string)
                        .collect();
                    v
                })
                .unwrap_or_else(|_| LAYOUT_DEFAULT.iter().map(|s| s.to_string()).collect());
            let icon_size = kf.int64(g.as_str(), "icon-size").unwrap_or(24) as i32;
            let opacity = kf.int64(g.as_str(), "opacity").unwrap_or(100) as i32;
            let color = kf
                .string(g, "color")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let monitor = kf
                .string(g, "monitor")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let workspace_max = kf.int64(g.as_str(), "workspace-max").unwrap_or(0) as i32;
            let align = match kf.string(g, "align").ok().as_deref() {
                Some("left") => Align::Left,
                Some("right") => Align::Right,
                _ => Align::Center,
            };
            let anim = kf.boolean(g, "anim").unwrap_or(false);
            let anim_colors: Vec<String> = kf
                .string(g, "anim-colors")
                .map(|raw| {
                    raw.split(';')
                        .filter(|e| gtk4::gdk::RGBA::parse(*e).is_ok())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_else(|_| BarConfig::default().anim_colors);
            let anim_dir = kf
                .string(g, "anim-dir")
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "ltr".into());
            let anim_speed = kf.int64(g, "anim-speed").unwrap_or(4) as i32;
            let anim_fps = kf.int64(g, "anim-fps").unwrap_or(30) as i32;
            Some(BarConfig {
                edge,
                height,
                width_percent,
                align,
                layout,
                icon_size,
                opacity,
                color,
                monitor,
                workspace_max,
                index: g
                    .strip_prefix("bar-")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0),
                anim,
                anim_colors,
                anim_dir,
                anim_speed,
                anim_fps,
            })
        })
        .collect()
}

/// Persist one bar's config under its group name (e.g. "bar-0").
pub fn save(index: usize, cfg: &BarConfig) {
    let group = format!("bar-{index}");
    let kf = glib::KeyFile::new();
    let _ = kf.load_from_file(path(), glib::KeyFileFlags::NONE);
    kf.set_string(&group, "edge", &cfg.edge);
    kf.set_int64(&group, "height", cfg.height as i64);
    kf.set_int64(&group, "width-percent", cfg.width_percent as i64);
    kf.set_string(
        &group,
        "align",
        match cfg.align {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        },
    );
    kf.set_string(&group, "layout", &cfg.layout.join(";"));
    kf.set_int64(&group, "icon-size", cfg.icon_size as i64);
    kf.set_int64(&group, "opacity", cfg.opacity as i64);
    kf.set_string(&group, "color", &cfg.color);
    kf.set_string(&group, "monitor", &cfg.monitor);
    kf.set_int64(&group, "workspace-max", cfg.workspace_max as i64);
    kf.set_boolean(&group, "anim", cfg.anim);
    kf.set_string(&group, "anim-colors", &cfg.anim_colors.join(";"));
    kf.set_string(&group, "anim-dir", &cfg.anim_dir);
    kf.set_int64(&group, "anim-speed", cfg.anim_speed as i64);
    kf.set_int64(&group, "anim-fps", cfg.anim_fps as i64);
    let dir = path().parent().unwrap().to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let _ = kf.save_to_file(path());
}
