// SPDX-License-Identifier: GPL-3.0-or-later
//! AppStream DEP-11 catalog reader.
//!
//! Debian ships per-origin component metadata as gzipped YAML under
//! /var/lib/swcatalog/yaml/. We extract only Type: desktop-application
//! components — that is the real-app filter: library packages and
//! dev files never carry a desktop entry, so the catalogue contains
//! actual programs only. The raw package index remains reachable
//! through the package search.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};

use flate2::read::GzDecoder;

const CATALOG_DIR: &str = "/var/lib/swcatalog/yaml";
/// Flathub's AppStream collection lives here once the remote exists.
// system-scope flatpak appstream (user scope resolved per-home below)
const FLATPAK_DIRS: &[&str] = &["/var/lib/flatpak/appstream"];

/// One application entry from the AppStream collection.
#[derive(Debug, Clone)]
pub struct AppItem {
    /// AppStream component id (e.g. "org.videolan.VLC.desktop").
    pub id: String,
    /// Human-readable name from Name.C.
    pub name: String,
    /// Debian package name (installable id).
    pub package: String,
    /// One-line description from Summary.C.
    pub summary: String,
    /// Desktop-entry category names (e.g. "AudioVideo", "Player").
    pub categories: Vec<String>,
    /// Source of this entry: "debian" or "flatpak".
    pub source: String,
    /// Long description text (markup stripped).
    pub desc: String,
    /// Icon reference — absolute png path for flatpak (local
    /// appstream cache), theme icon name for debian (DEP-11 stock).
    pub icon: String,
}

/// Load every desktop-application component from the DEP-11 yaml.gz
/// collection files. Missing/undecodable files are skipped so a
/// partially populated cache still yields a usable catalogue.
pub fn load_apps() -> Vec<AppItem> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(CATALOG_DIR) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("gz") {
            continue;
        }
        let Ok(f) = File::open(&p) else { continue };
        parse_yaml(&mut GzDecoder::new(f), &mut out);
    }
    // flathub trees, system + user scope (~/.local/share/flatpak)
    let mut dirs: Vec<String> = FLATPAK_DIRS.iter().map(|d| d.to_string()).collect();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/.local/share/flatpak/appstream"));
    }
    for dir in &dirs {
        collect_flatpak(dir, &mut out);
    }
    // same remote exists in system + user scope — drop dupes
    out.sort_by_key(|a| (a.id.clone(), a.source.clone()));
    out.dedup_by(|a, b| a.id == b.id && a.source == b.source);
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Walk flatpak appstream dirs for yaml.gz files, tag source=flatpak.
fn collect_flatpak(dir: &str, out: &mut Vec<AppItem>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_flatpak(p.to_str().unwrap_or(""), out);
        } else if p.to_string_lossy().ends_with(".xml.gz") {
            if let Ok(f) = File::open(&p) {
                let start = out.len();
                parse_xml(&mut GzDecoder::new(f), out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        } else if p.to_string_lossy().ends_with("appstream.xml") {
            // flatpak also stores the collection uncompressed
            if let Ok(mut f) = File::open(&p) {
                let start = out.len();
                parse_xml(&mut f, out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        } else if p.to_string_lossy().ends_with(".yml.gz")
            || p.to_string_lossy().ends_with(".yaml.gz")
        {
            if let Ok(f) = File::open(&p) {
                let start = out.len();
                parse_yaml(&mut GzDecoder::new(f), out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        }
    }
}

/// Parse one DEP-11 stream. Documents are separated by `---`; fields
/// we need sit on single lines except localized maps where only the
/// `C:` (default locale) entry is taken.
fn parse_yaml(r: &mut impl Read, out: &mut Vec<AppItem>) {
    let mut cur = AppItem::empty();
    let mut in_name = false;
    let mut in_summary = false;
    let mut in_categories = false;
    let mut in_desc = false;
    let mut in_icon = false;
    let mut desc_block = false;
    let mut skip_block = false;

    for line in BufReader::new(r).lines().map_while(Result::ok) {
        if line == "---" {
            cur.finish(out);
            cur = AppItem::empty();
            in_name = false;
            in_summary = false;
            in_categories = false;
            in_icon = false;
            skip_block = false;
            continue;
        }
        // DEP-11 list items sit at column 0 ("- Game") — capture
        // them before the key-line reset swallows the flag.
        if in_categories && line.trim_start().starts_with("- ") {
            cur.categories.push(unquote(&line.trim_start()[2..]));
            continue;
        }
        if line.starts_with(|c: char| !c.is_whitespace()) {
            in_name = false;
            in_summary = false;
            in_categories = false;
            in_icon = false;
            skip_block = false;
            if let Some((k, v)) = line.split_once(':') {
                match k {
                    "Type" => skip_block = v.trim() != "desktop-application",
                    "ID" => cur.id = unquote(v),
                    "Package" => cur.package = unquote(v),
                    "Name" => in_name = true,
                    "Summary" => in_summary = true,
                    "Categories" => in_categories = true,
                    "Description" => in_desc = true,
                    "Icon" => in_icon = true,
                    _ => {}
                }
            }
            continue;
        }
        if skip_block {
            continue;
        }
        let t = line.trim_start();
        // DEP-11 Icon block — the stock name is a theme icon we can
        // resolve locally; cached/remote files are not downloaded
        if in_icon && t.starts_with("stock:") {
            cur.icon = unquote(&t[6..]);
            in_icon = false;
        }
        if in_name && t.starts_with("C:") {
            cur.name = unquote(&t[2..]);
            in_name = false;
        } else if in_summary && t.starts_with("C:") {
            cur.summary = unquote(&t[2..]);
            in_summary = false;
        } else if in_desc && t.starts_with("C:") {
            let v = t[2..].trim();
            if v == "|-" || v == "|" || v == ">-" || v == ">" {
                // yaml block scalar — capture indented lines below
                desc_block = true;
                cur.desc.clear();
            } else {
                cur.desc = strip_tags(v);
            }
            in_desc = false;
        } else if desc_block {
            if line.trim().is_empty()
                || t.contains(':')
                    && t.split(':')
                        .next()
                        .map(|k| k.chars().all(|c| c.is_alphanumeric() || c == '_'))
                        .unwrap_or(false)
            {
                // next localized key (e.g. "  es:") or blank → stop
                desc_block = false;
            } else {
                cur.desc.push(' ');
                cur.desc.push_str(&strip_tags(t.trim()));
                continue;
            }
        }
        if in_categories && t.starts_with("- ") {
            cur.categories.push(unquote(&t[2..]));
        }
    }
    cur.finish(out);
}

/// Strip surrounding quotes yaml puts around values.
fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

impl AppItem {
    fn empty() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            package: String::new(),
            summary: String::new(),
            categories: Vec::new(),
            source: "debian".into(),
            desc: String::new(),
            icon: String::new(),
        }
    }

    /// Push a finished component — must have name + package and be a
    /// real application (the Type filter already ran).
    fn finish(self, out: &mut Vec<AppItem>) {
        if !self.name.is_empty() && !self.package.is_empty() {
            out.push(self);
        }
    }
}

/// Names of all installed packages via dpkg-query — single call,
/// cheap enough for a badge lookup.
pub fn installed_set() -> std::collections::HashSet<String> {
    let mut s = std::collections::HashSet::new();
    if let Ok(out) = std::process::Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\n"])
        .output()
    {
        for l in String::from_utf8_lossy(&out.stdout).lines() {
            s.insert(l.trim().to_string());
        }
    }
    s
}

/// Ensure the flathub remote exists (user scope, no root needed).
pub fn ensure_flathub() {
    let has = std::process::Command::new("flatpak")
        .args(["remotes", "--user", "--columns=name"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("flathub"))
        .unwrap_or(false);
    if !has {
        let _ = std::process::Command::new("flatpak")
            .args([
                "remote-add",
                "--user",
                "--if-not-exists",
                "flathub",
                "https://dl.flathub.org/repo/flathub.flatpakrepo",
            ])
            .output();
    }
}

/// Installed flatpak app ids (app/… form) for the badge check.
pub fn flatpak_installed() -> std::collections::HashSet<String> {
    let mut s = std::collections::HashSet::new();
    if let Ok(o) = std::process::Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
    {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            s.insert(l.trim().to_string());
        }
    }
    s
}

/// Minimal AppStream collection-XML parser: pulls id/name/summary/
/// pkgname/categories out of <component type="desktop-application">
/// blocks. The un-languaged element wins for name/summary.
fn parse_xml(r: &mut impl Read, out: &mut Vec<AppItem>) {
    let mut buf = String::new();
    if r.read_to_string(&mut buf).is_err() {
        return;
    }
    for block in buf.split("<component") {
        if !block.contains("type=\"desktop-application\"") {
            continue;
        }
        let mut a = AppItem::empty();
        a.source = "flatpak".into();
        if let Some(v) = tag(block, "id") {
            a.id = v;
        }
        if let Some(v) = tag(block, "pkgname") {
            a.package = v;
        }
        // flathub xml has no pkgname — the component id IS the ref
        if a.package.is_empty() {
            a.package = a.id.clone();
        }
        if let Some(v) = tag(block, "name") {
            a.name = v;
        }
        if let Some(v) = tag(block, "summary") {
            a.summary = v;
        }
        if let Some(v) = tag(block, "description") {
            a.desc = strip_tags(&v);
        }
        let mut rest = block;
        while let Some(pos) = rest.find("<category>") {
            rest = &rest[pos + 10..];
            if let Some(end) = rest.find("</category>") {
                a.categories.push(rest[..end].to_string());
            }
        }
        // flathub caches appstream icons locally, named by app id —
        // use them only when the file actually exists
        let icon_path = format!(
            "/var/lib/flatpak/appstream/flathub/x86_64/active/icons/64x64/{}.png",
            a.id.trim_end_matches(".desktop")
        );
        if std::path::Path::new(&icon_path).exists() {
            a.icon = icon_path;
        }
        a.finish(out);
    }
}

/// First occurrence of <tag>…</tag> without xml:lang attribute.
fn tag(block: &str, t: &str) -> Option<String> {
    let open = format!("<{t}>");
    let close = format!("</{t}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

/// Remove simple XML/HTML markup from description bodies.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
