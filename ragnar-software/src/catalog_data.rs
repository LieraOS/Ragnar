// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
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

/// Language preference list for localised AppStream fields —
/// e.g. ["de_DE", "de"] on a German system, empty on English (the
/// un-languaged `C:` fields already are English). glib resolves
/// LANG/LANGUAGE for us.
fn locale_candidates() -> Vec<String> {
    let mut out = Vec::new();
    for l in gtk4::glib::language_names() {
        let l = l.replace('-', "_");
        if l == "C" || l.starts_with("en") {
            continue;
        }
        out.push(l.clone());
        if let Some(base) = l.split('_').next() {
            if base != l {
                out.push(base.to_string());
            }
        }
    }
    out
}

/// Does the localised map key `k` match one of the wanted locales?
fn wanted_key<'a>(k: &str, want: &'a [String]) -> Option<&'a str> {
    want.iter()
        .find(|w| w.as_str() == k)
        .map(|w| w.as_str())
}

/// Load every desktop-application component from the DEP-11 yaml.gz
/// collection files. Missing/undecodable files are skipped so a
/// partially populated cache still yields a usable catalogue.
/// Name/Summary/Description prefer the system-locale entry and fall
/// back to the un-languaged `C:` text.
pub fn load_apps() -> Vec<AppItem> {
    let want = locale_candidates();
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
        parse_yaml(&mut GzDecoder::new(f), &want, &mut out);
    }
    // flathub trees, system + user scope (~/.local/share/flatpak)
    let mut dirs: Vec<String> = FLATPAK_DIRS.iter().map(|d| d.to_string()).collect();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/.local/share/flatpak/appstream"));
    }
    for dir in &dirs {
        collect_flatpak(dir, &want, &mut out);
    }
    // same remote exists in system + user scope — drop dupes
    out.sort_by_key(|a| (a.id.clone(), a.source.clone()));
    out.dedup_by(|a, b| a.id == b.id && a.source == b.source);
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Walk flatpak appstream dirs for xml/yaml files, tag source=flatpak.
fn collect_flatpak(dir: &str, want: &[String], out: &mut Vec<AppItem>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_flatpak(p.to_str().unwrap_or(""), want, out);
        } else if p.to_string_lossy().ends_with(".xml.gz") {
            if let Ok(f) = File::open(&p) {
                let start = out.len();
                parse_xml(&mut GzDecoder::new(f), want, out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        } else if p.to_string_lossy().ends_with("appstream.xml") {
            // flatpak also stores the collection uncompressed
            if let Ok(mut f) = File::open(&p) {
                let start = out.len();
                parse_xml(&mut f, want, out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        } else if p.to_string_lossy().ends_with(".yml.gz")
            || p.to_string_lossy().ends_with(".yaml.gz")
        {
            if let Ok(f) = File::open(&p) {
                let start = out.len();
                parse_yaml(&mut GzDecoder::new(f), want, out);
                for a in &mut out[start..] {
                    a.source = "flatpak".into();
                }
            }
        }
    }
}

/// Parse one DEP-11 stream. Documents are separated by `---`.
/// Localised maps (Name/Summary/Description) yield two values per
/// component: the un-languaged `C:` fallback and the first entry
/// matching the system locale — finish() prefers the localised one.
fn parse_yaml(r: &mut impl Read, want: &[String], out: &mut Vec<AppItem>) {
    let mut cur = AppItem::empty();
    let mut name_loc: Option<String> = None;
    let mut sum_loc: Option<String> = None;
    let mut desc_loc = String::new();
    let mut in_name = false;
    let mut in_summary = false;
    let mut in_categories = false;
    let mut in_desc = false;
    let mut in_icon = false;
    // Which buffer an open |-/| description block feeds:
    // 0 none, 1 = C fallback, 2 = system-locale override.
    let mut desc_target = 0u8;
    let mut skip_block = false;

    for line in BufReader::new(r).lines().map_while(Result::ok) {
        if line == "---" {
            cur.finish(out, name_loc.take(), sum_loc.take(), desc_loc.split_off(0));
            cur = AppItem::empty();
            in_name = false;
            in_summary = false;
            in_categories = false;
            in_desc = false;
            in_icon = false;
            desc_target = 0;
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
            in_desc = false;
            in_icon = false;
            desc_target = 0;
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
        // Localised single-line values — C: is the fallback and
        // keeps the flag open so a matching locale entry later in
        // the same map still overrides it.
        if in_name {
            if let Some(v) = t.strip_prefix("C:") {
                cur.name = unquote(v);
            } else if name_loc.is_none() {
                if let Some((k, v)) = t.split_once(':') {
                    if wanted_key(k.trim(), want).is_some() {
                        name_loc = Some(unquote(v));
                    }
                }
            }
        }
        if in_summary {
            if let Some(v) = t.strip_prefix("C:") {
                cur.summary = unquote(v);
            } else if sum_loc.is_none() {
                if let Some((k, v)) = t.split_once(':') {
                    if wanted_key(k.trim(), want).is_some() {
                        sum_loc = Some(unquote(v));
                    }
                }
            }
        }
        if in_desc {
            if desc_target != 0 {
                // Inside a folded block: content is indented deeper
                // than the 2-space locale keys — a key-level line
                // closes the block and is re-processed below.
                let key_level = line.starts_with("  ") && !line.starts_with("   ");
                if !key_level {
                    let d = if desc_target == 2 {
                        &mut desc_loc
                    } else {
                        &mut cur.desc
                    };
                    let s = strip_tags(t.trim());
                    if !s.is_empty() {
                        if !d.is_empty() {
                            d.push(' ');
                        }
                        d.push_str(&s);
                    }
                    continue;
                }
                desc_target = 0;
            }
            if let Some((k, v)) = t.split_once(':') {
                let (k, v) = (k.trim(), v.trim());
                let mut tgt = if k == "C" {
                    1u8
                } else if wanted_key(k, want).is_some() {
                    2
                } else {
                    0
                };
                // First matching locale wins — a second one must not
                // overwrite what an earlier wanted key already set.
                if tgt == 2 && !desc_loc.is_empty() {
                    tgt = 0;
                }
                if tgt != 0 {
                    if matches!(v, "|-" | "|" | ">-" | ">") {
                        desc_target = tgt;
                    } else if !v.is_empty() {
                        let d = if tgt == 2 {
                            &mut desc_loc
                        } else {
                            &mut cur.desc
                        };
                        *d = strip_tags(v);
                    }
                }
            }
        }
        if in_categories && t.starts_with("- ") {
            cur.categories.push(unquote(&t[2..]));
        }
    }
    cur.finish(out, name_loc, sum_loc, desc_loc);
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
    /// real application (the Type filter already ran). Localised
    /// values override the C: fallback when present.
    fn finish(
        mut self,
        out: &mut Vec<AppItem>,
        name_loc: Option<String>,
        sum_loc: Option<String>,
        desc_loc: String,
    ) {
        if let Some(n) = name_loc {
            self.name = n;
        }
        if let Some(s) = sum_loc {
            self.summary = s;
        }
        if !desc_loc.is_empty() {
            self.desc = desc_loc;
        }
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
fn parse_xml(r: &mut impl Read, want: &[String], out: &mut Vec<AppItem>) {
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
        if let Some(v) = tag_loc(block, "name", want) {
            a.name = v;
        }
        if let Some(v) = tag_loc(block, "summary", want) {
            a.summary = v;
        }
        if let Some(v) = tag_loc(block, "description", want) {
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
        a.finish(out, None, None, String::new());
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

/// Localised element <tag xml:lang="…"> — tries each wanted locale
/// (both de_DE and de-DE spellings appear in the wild), falls back
/// to the un-languaged element.
fn tag_loc(block: &str, t: &str, want: &[String]) -> Option<String> {
    for w in want {
        for lang in [w.as_str(), &w.replace('_', "-")] {
            let open = format!("<{t} xml:lang=\"{lang}\">");
            let close = format!("</{t}>");
            if let Some(s) = block.find(&open) {
                let s = s + open.len();
                if let Some(e) = block[s..].find(&close) {
                    return Some(block[s..s + e].trim().to_string());
                }
            }
        }
    }
    tag(block, t)
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
