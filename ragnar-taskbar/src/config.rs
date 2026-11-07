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
    let dir = path().parent().unwrap().to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let _ = kf.save_to_file(path());
}
