// Start-menu configuration — persisted next to bars.ini as menu.ini.
// Stores the category switch mode (hover vs click), whether the
// category column shows at all, and user-defined custom categories
// (name + the desktop ids assigned to them). Read fresh on every
// menu open so settings take effect immediately.
use gtk4::glib;

const CFG_FILE: &str = "menu.ini";

/// One user-defined category: display name plus the desktop-file
/// ids ("firefox.desktop") the user assigned to it.
#[derive(Clone)]
pub struct CustomCat {
    pub name: String,
}

#[derive(Clone)]
pub struct MenuCfg {
    /// true = categories switch on hover (default); false = on click.
    pub switch_hover: bool,
    /// Show the category column at all (false = flat app list).
    pub show_categories: bool,
    /// User-created category names (existence only — membership
    /// lives in `map`).
    pub custom: Vec<CustomCat>,
    /// Per-app overrides: desktop id -> category name. Wins over the
    /// app's own .desktop category; works for standard AND custom
    /// categories.
    pub map: std::collections::HashMap<String, String>,
    /// Per-app display renames: desktop id -> custom label.
    pub renames: std::collections::HashMap<String, String>,
    /// Desktop ids hidden from the start menu entirely.
    pub hidden: std::collections::HashSet<String>,
    /// Display order of categories by their display name (standard
    /// labels and custom names mixed). Empty = built-in order.
    pub cat_order: Vec<String>,
}

impl Default for MenuCfg {
    fn default() -> Self {
        Self {
            switch_hover: true,
            show_categories: true,
            custom: Vec::new(),
            map: std::collections::HashMap::new(),
            renames: std::collections::HashMap::new(),
            hidden: std::collections::HashSet::new(),
            cat_order: Vec::new(),
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::config::cfg_dir().join(CFG_FILE)
}

/// Load the menu config; defaults apply when the file is absent.
pub fn load() -> MenuCfg {
    let kf = glib::KeyFile::new();
    let mut cfg = MenuCfg::default();
    if kf.load_from_file(path(), glib::KeyFileFlags::NONE).is_err() {
        return cfg;
    }
    cfg.switch_hover = kf.boolean("menu", "switch_hover").unwrap_or(true);
    cfg.show_categories = kf.boolean("menu", "categories").unwrap_or(true);
    // Custom categories live in groups named [cat:<name>].
    for g in kf.groups() {
        if let Some(name) = g.strip_prefix("cat:") {
            cfg.custom.push(CustomCat {
                name: name.to_string(),
            });
        } else if g == "map" {
            for key in kf.keys(&g).unwrap_or_default() {
                if let Ok(v) = kf.string(&g, &key) {
                    cfg.map.insert(key.to_string(), v.to_string());
                }
            }
        } else if g == "rename" {
            for key in kf.keys(&g).unwrap_or_default() {
                if let Ok(v) = kf.string(&g, &key) {
                    cfg.renames.insert(key.to_string(), v.to_string());
                }
            }
        } else if g == "hidden" {
            for key in kf.keys(&g).unwrap_or_default() {
                if kf.boolean(&g, &key).unwrap_or(false) {
                    cfg.hidden.insert(key.to_string());
                }
            }
        } else if g == "order" {
            cfg.cat_order = kf
                .string(&g, "list")
                .map(|s| s.split(';').map(|x| x.to_string()).collect())
                .unwrap_or_default();
        }
    }
    cfg
}

/// Persist the whole menu config — the file is small enough that a
/// full rewrite is simpler than diffing groups.
pub fn save(cfg: &MenuCfg) {
    let kf = glib::KeyFile::new();
    kf.set_boolean("menu", "switch_hover", cfg.switch_hover);
    kf.set_boolean("menu", "categories", cfg.show_categories);
    for cat in &cfg.custom {
        // An otherwise empty group needs one key to persist.
        kf.set_string(&format!("cat:{}", cat.name), "exists", "1");
    }
    for (id, cat) in &cfg.map {
        kf.set_string("map", id, cat);
    }
    for (id, name) in &cfg.renames {
        kf.set_string("rename", id, name);
    }
    for id in &cfg.hidden {
        kf.set_boolean("hidden", id, true);
    }
    if !cfg.cat_order.is_empty() {
        kf.set_string("order", "list", &cfg.cat_order.join(";"));
    }
    let dir = path().parent().unwrap().to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let _ = kf.save_to_file(path());
}
