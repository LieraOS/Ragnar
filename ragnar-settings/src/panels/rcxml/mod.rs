// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor configuration file patcher
// Copyright (C) 2026 LieraOS Project
//
// Text-level editor for the compositor's rc.xml. The file is the
// decoupling interface: panels write values, the compositor reloads
// them — no implementation-specific API is assumed anywhere.
//
// The patcher works on whole-element text: `<key>v</key>` inside a
// named block is replaced in place, appended before the block's
// closing tag when absent, and missing top-level sections are
// created before `</labwc_config>`. Surrounding content — keybinds,
// window rules, comments — is never touched.

use std::path::PathBuf;

/// Path to the compositor configuration file.
pub fn rc_path() -> PathBuf {
    let mut p = gtk4::glib::user_config_dir();
    p.push("ragnar-desktop");
    p.push("labwc");
    p.push("rc.xml");
    p
}

/// Read the whole file (empty string when unreadable).
pub fn read() -> String {
    std::fs::read_to_string(rc_path()).unwrap_or_default()
}

/// Ask the compositor to reload its configuration — edits go live
/// without a session restart. `labwc --reconfigure` SIGHUPs the
/// running compositor; when the binary call fails, the signal is
/// sent directly so a change never silently stays unapplied.
pub fn reload() {
    let ok = std::process::Command::new("labwc")
        .arg("--reconfigure")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let fallback = std::process::Command::new("pkill")
            .args(["-HUP", "-x", "labwc"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !fallback {
            tracing::warn!("compositor reload failed: no labwc process answered");
        }
    }
}

// -------------------------------------------------------------------
// Block-level get/set — the generic API for all sections
// -------------------------------------------------------------------

/// Read `<key>v</key>` inside the block opened by `open_tag`
/// (e.g. `"<theme>"` or `"<font place=\"ActiveWindow\">"`), closed
/// by `close_tag`.
pub fn get_in(open_tag: &str, close_tag: &str, key: &str) -> Option<String> {
    let t = read();
    let (a, b) = block_span(&t, open_tag, close_tag)?;
    get_tag(&t[a..b], key)
}

/// Write `<key>val</key>` inside the `open_tag .. close_tag` block.
/// When the block is missing and `parent` names its top-level
/// section (e.g. "theme"), that section is created with the child
/// block inside.
pub fn set_in(open_tag: &str, close_tag: &str, key: &str, val: &str, parent: Option<&str>) {
    let mut t = read();
    if t.is_empty() {
        return;
    }
    if block_span(&t, open_tag, close_tag).is_none() {
        ensure_block(&mut t, open_tag, close_tag, parent);
    }
    if let Some((a, b)) = block_span(&t, open_tag, close_tag) {
        set_tag(&mut t, a, b, key, val);
    }
    let _ = std::fs::write(rc_path(), t);
}

/// Top-level convenience: `<section><key>v</key></section>`.
/// Creates the section when missing.
pub fn set_section(section: &str, key: &str, val: &str) {
    set_in(
        &format!("<{section}>"),
        &format!("</{section}>"),
        key,
        val,
        Some(section),
    );
}

/// Top-level convenience read for `<section><key>v</key></section>`.
pub fn get_section(section: &str, key: &str) -> Option<String> {
    get_in(&format!("<{section}>"), &format!("</{section}>"), key)
}

/// Write `attr="val"` on the first element matching `<tag …` —
/// `tag` may itself carry attributes to disambiguate repeated
/// elements (e.g. `font place="ActiveWindow"`). The attribute is
/// inserted when absent; a missing element is created before
/// `</labwc_config>`.
pub fn set_attr(tag: &str, attr: &str, val: &str) {
    let mut t = read();
    if t.is_empty() {
        return;
    }
    if t.find(&format!("<{tag}")).is_none() {
        if let Some(end) = t.find("</labwc_config>") {
            // Self-closing, attribute already in place — a missing
            // element carries no body the caller did not ask for.
            t.insert_str(end, &format!("\t<{tag} {attr}=\"{val}\"/>\n"));
        }
    }
    let Some(a) = t.find(&format!("<{tag}")) else {
        return;
    };
    let Some(gt) = t[a..].find('>').map(|i| a + i) else {
        return;
    };
    let head = &t[a..gt];
    let pat = format!("{attr}=\"");
    if let Some(p) = head.find(&pat) {
        // Replace the existing attribute value.
        let vs = a + p + pat.len();
        if let Some(ve) = t[vs..gt].find('"') {
            t.replace_range(vs..vs + ve, val);
        }
    } else {
        // Self-closing `<tag />` — the attribute goes before the
        // `/`, reusing the whitespace that already separates it.
        let trimmed = head.trim_end().len();
        let ins = if head[..trimmed].ends_with('/') {
            a + trimmed - 1
        } else {
            gt
        };
        let pre = if t.as_bytes()[ins - 1].is_ascii_whitespace() {
            ""
        } else {
            " "
        };
        t.insert_str(ins, &format!("{pre}{attr}=\"{val}\""));
    }
    let _ = std::fs::write(rc_path(), t);
}

/// Read `attr="val"` from the first element matching `<tag …`
/// (same disambiguation as `set_attr`).
pub fn get_attr(tag: &str, attr: &str) -> Option<String> {
    let t = read();
    let a = t.find(&format!("<{tag}"))?;
    let gt = t[a..].find('>').map(|i| a + i)?;
    let head = &t[a..gt];
    let pat = format!("{attr}=\"");
    let p = head.find(&pat)? + pat.len();
    let e = head[p..].find('"')?;
    Some(head[p..p + e].to_string())
}

// -------------------------------------------------------------------
// Input-device + keyboard helpers (used by the input panel)
// -------------------------------------------------------------------

/// Write a key inside `<device category="cat">…</device>`.
pub fn set_device(cat: &str, key: &str, val: &str) {
    set_in(
        &format!("<device category=\"{cat}\">"),
        "</device>",
        key,
        val,
        Some("libinput"),
    );
}

/// Write a key inside `<keyboard>…</keyboard>`.
pub fn set_keyboard(key: &str, val: &str) {
    set_in("<keyboard>", "</keyboard>", key, val, Some("keyboard"));
}

/// Read a `<key>` value inside a `<device category>` block.
pub fn get_device(cat: &str, key: &str) -> Option<String> {
    get_in(&format!("<device category=\"{cat}\">"), "</device>", key)
}

/// Read a `<key>` value inside `<keyboard>`.
pub fn get_keyboard(key: &str) -> Option<String> {
    get_in("<keyboard>", "</keyboard>", key)
}

/// Read `<desktops><names><name>…` entries in order. `<name>`
/// elements are positional — index i names workspace i+1.
pub fn get_names() -> Vec<String> {
    let t = read();
    let Some((a, b)) = block_span(&t, "<names>", "</names>") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut rest = &t[a..b];
    while let Some(s) = rest.find("<name>") {
        let seg = &rest[s + "<name>".len()..];
        let Some(e) = seg.find("</name>") else {
            break;
        };
        out.push(seg[..e].to_string());
        rest = &seg[e..];
    }
    out
}

/// Replace `<desktops><names>` with one `<name>` per entry.
/// Empty slots get the generated "<prefix> N" label — positional
/// names mean a gap would otherwise render as a blank name.
/// Creates the `<names>` block (or the whole `<desktops>` section)
/// when missing.
pub fn set_names(names: &[String]) {
    let mut t = read();
    if t.is_empty() {
        return;
    }
    let prefix = block_span(&t, "<desktops", "</desktops>")
        .and_then(|(a, b)| get_tag(&t[a..b], "prefix"))
        .unwrap_or_else(|| "Workspace".to_string());
    let mut block = String::from("\t\t<names>\n");
    for (i, n) in names.iter().enumerate() {
        let label = match n.trim() {
            "" => format!("{prefix} {}", i + 1),
            x => x.to_string(),
        };
        block.push_str(&format!("\t\t\t<name>{label}</name>\n"));
    }
    block.push_str("\t\t</names>\n");

    if let Some((a, b)) = block_span(&t, "<names>", "</names>") {
        // Replace the whole element plus its trailing newline.
        let mut end = b + "</names>".len();
        if t[end..].starts_with('\n') {
            end += 1;
        }
        t.replace_range(a..end, &block);
    } else if let Some(ds) = t.find("<desktops") {
        let Some(gt) = t[ds..].find('>').map(|i| ds + i) else {
            return;
        };
        if t[ds..gt].trim_end().ends_with('/') {
            // <desktops … /> → expand into a full element.
            let open = t[ds..gt].trim_end().trim_end_matches('/').to_string();
            let open = format!("{open}>");
            t.replace_range(ds..gt + 1, &format!("{open}\n{block}\t</desktops>\n"));
        } else {
            t.insert_str(gt + 1, &format!("\n{block}"));
        }
    } else if let Some(end) = t.find("</labwc_config>") {
        t.insert_str(end, &format!("\t<desktops>\n{block}\t</desktops>\n"));
    }
    let _ = std::fs::write(rc_path(), t);
    reload();
}

// -------------------------------------------------------------------
// Text surgery internals
// -------------------------------------------------------------------

/// Byte span (open-tag start .. close-tag start) of a block.
fn block_span(t: &str, open_tag: &str, close_tag: &str) -> Option<(usize, usize)> {
    let a = t.find(open_tag)?;
    let b = t[a..].find(close_tag).map(|i| a + i)?;
    Some((a, b))
}

/// Insert a missing block: at top level inside `<labwc_config>`
/// when `parent` is given and absent, or directly before
/// `</labwc_config>` otherwise. When `parent` exists but the child
/// block is missing, the child is appended inside the parent.
fn ensure_block(t: &mut String, open_tag: &str, close_tag: &str, parent: Option<&str>) {
    // Extract the element name from "<name ...>" → "name".
    let name = open_tag
        .trim_start_matches('<')
        .split([' ', '>'])
        .next()
        .unwrap_or("");
    if name.is_empty() {
        return;
    }
    if let Some(section) = parent {
        // When the block IS the section itself, skip the parent
        // lookup — otherwise we'd create <section><section>.
        if section != name {
            let sp = format!("<{section}>");
            let ep = format!("</{section}>");
            if block_span(t, &sp, &ep).is_none() {
                // Whole section missing — create it with the child
                // block inside, before the document root closes.
                let block = format!("\t<{section}>\n\t\t{open_tag}\n\t\t{close_tag}\n\t{ep}\n");
                if let Some(end) = t.find("</labwc_config>") {
                    t.insert_str(end, &block);
                }
                return;
            }
        }
    }
    // Parent (or no parent needed) — append the empty child block
    // right before the relevant closing tag. A top-level section
    // (parent == own name) goes before the document root closes at
    // one level of indent; a nested child sits one level deeper.
    let (close_at, ind) = match parent {
        Some(section) if section != name => (t.find(&format!("</{section}>")), "\t\t"),
        _ => (t.find("</labwc_config>"), "\t"),
    };
    if let Some(pos) = close_at {
        t.insert_str(pos, &format!("{ind}{open_tag}\n{ind}{close_tag}\n"));
    }
}

/// Replace `<key>v</key>` inside `t[a..b]`; append it before the
/// block end when absent.
fn set_tag(t: &mut String, a: usize, b: usize, key: &str, val: &str) {
    let block = &t[a..b];
    let open = format!("<{key}>");
    if let Some(k) = block.find(&open) {
        let val_start = a + k + open.len();
        let close = format!("</{key}>");
        if let Some(rel) = t[val_start..b].find(&close) {
            t.replace_range(val_start..val_start + rel, val);
            return;
        }
    }
    // Indent one level deeper than the block's own closing tag, so
    // new elements line up with their siblings. Only trust the line
    // prefix when it is pure indentation; inline XML falls back.
    let line_start = t[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prefix = &t[line_start..b];
    let ind = if prefix.chars().all(char::is_whitespace) {
        format!("{prefix}\t")
    } else {
        "\t\t".to_string()
    };
    t.insert_str(b, &format!("{ind}<{key}>{val}</{key}>\n"));
}

/// Extract `<key>value</key>` from a block snippet.
fn get_tag(block: &str, key: &str) -> Option<String> {
    let open = format!("<{key}>");
    let close = format!("</{key}>");
    let a = block.find(&open)? + open.len();
    let b = block[a..].find(&close).map(|i| a + i)?;
    Some(block[a..b].trim().to_string())
}

#[cfg(test)]
#[cfg(test)]
mod tests;
