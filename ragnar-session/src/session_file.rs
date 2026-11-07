//! Parser for `.session` files (GLib key-file format).
//!
//! A `.session` file declares the session name and the list of
//! *required components* — desktop-entry IDs that the session manager
//! must launch and (if `X-GNOME-AutoRestart` is set) respawn on crash.

use std::path::PathBuf;

/// One required component as declared in the `.session` file.
#[derive(Debug, Clone)]
pub struct RequiredComponent {
    /// Desktop-entry ID (e.g. `org.lieraos.ragnar.PowerDialog`).
    pub id: String,
}

/// Parsed content of a `.session` file.
#[derive(Debug, Clone)]
pub struct SessionFile {
    /// Human-readable session name (falls back to the file name).
    pub name: String,
    /// Required component desktop-entry IDs, in declaration order.
    pub required: Vec<RequiredComponent>,
}

impl SessionFile {
    /// Loads and parses a `.session` file from disk.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut name = String::new();
        let mut required = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "Name" => name = value.trim().to_string(),
                    // RequiredComponents carries ';'-separated IDs that
                    // may contain spaces; strip empties and whitespace.
                    "RequiredComponents" => {
                        required = value
                            .split(';')
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| RequiredComponent { id: s.to_string() })
                            .collect();
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Ragnar".to_string());
        }
        Ok(Self { name, required })
    }
}

/// Searches the XDG applications and autostart directories for a
/// desktop entry with the given ID and returns its path.
///
/// Lookup order: `$XDG_CONFIG_HOME/autostart`,
/// `/etc/xdg/autostart`, `$XDG_DATA_HOME/applications`,
/// `$XDG_DATA_DIRS/applications`.
pub fn find_desktop_file(id: &str) -> Option<PathBuf> {
    let file_name = format!("{id}.desktop");
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(home).join("autostart"));
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".config/autostart"));
    }
    dirs.push(PathBuf::from("/etc/xdg/autostart"));

    if let Ok(home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in data_dirs.split(':') {
        dirs.push(PathBuf::from(d).join("applications"));
    }

    dirs.iter().map(|d| d.join(&file_name)).find(|p| p.exists())
}

/// Extracts `Exec=` and `X-GNOME-AutoRestart=` from a desktop file.
pub fn parse_desktop_exec(path: &std::path::Path) -> Option<(String, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut exec = None;
    let mut restart = false;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(v) = line.strip_prefix("Exec=") {
            // Field codes (%U, %f, ...) are meaningless for session
            // components — strip them.
            let cmd = v
                .split_whitespace()
                .filter(|t| !t.starts_with('%'))
                .collect::<Vec<_>>()
                .join(" ");
            exec = Some(cmd);
        } else if line == "X-GNOME-AutoRestart=true" {
            restart = true;
        }
    }
    exec.map(|e| (e, restart))
}
