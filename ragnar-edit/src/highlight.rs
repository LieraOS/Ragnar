//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! Lightweight whole-buffer syntax highlighting via TextTags.
//! Language is chosen by file extension, matching the original
//! LieraEdit language set. Offsets inside TextBuffer are char-based,
//! so all positions are converted from byte indices.

use crate::keywords::{CML, CPP, JS, PHP, PY, RS, SH, SQL};
use gtk4::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    Plain,
    Cpp,
    Python,
    Js,
    Ts,
    Json,
    Xml,
    Html,
    Bash,
    Cmake,
    Docker,
    Yaml,
    Css,
    Md,
    Ini,
    Rs,
    Php,
    Sql,
}

/// Maps a file extension to a highlighting language.
pub fn from_ext(ext: &str) -> Lang {
    match ext.to_ascii_lowercase().as_str() {
        "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" => Lang::Cpp,
        "py" => Lang::Python,
        "js" | "jsx" | "mjs" => Lang::Js,
        "ts" | "tsx" => Lang::Ts,
        "json" => Lang::Json,
        "xml" | "xsl" | "svg" | "ui" => Lang::Xml,
        "html" | "htm" => Lang::Html,
        "sh" | "bash" | "zsh" => Lang::Bash,
        "cmake" => Lang::Cmake,
        "dockerfile" | "containerfile" => Lang::Docker,
        "yml" | "yaml" => Lang::Yaml,
        "css" | "scss" => Lang::Css,
        "md" | "markdown" => Lang::Md,
        "ini" | "cfg" | "conf" | "properties" | "env" | "desktop" | "toml" => Lang::Ini,
        "rs" => Lang::Rs,
        "php" => Lang::Php,
        "sql" => Lang::Sql,
        _ => Lang::Plain,
    }
}

/// (name, light-fg, dark-fg, bold, italic)
const PALETTE: &[(&str, &str, &str, bool, bool)] = &[
    ("kw", "#6f42c1", "#b392f0", true, false),
    ("str", "#a31515", "#f97583", false, false),
    ("cmt", "#22863a", "#85d06e", false, true),
    ("num", "#005cc5", "#79c0ff", false, false),
    ("hdr", "#24292e", "#f0f3f6", true, false),
    ("qte", "#22863a", "#85d06e", false, true),
    ("bld", "#24292e", "#f0f3f6", true, false),
    ("itl", "#24292e", "#e6edf3", false, true),
    ("lnk", "#0366d6", "#58a6ff", false, false),
    ("cde", "#d73a49", "#ff7b72", false, false),
];

fn ensure_tags(buf: &gtk4::TextBuffer) {
    let t = buf.tag_table();
    for (name, _, _, bold, italic) in PALETTE {
        if t.lookup(name).is_none() {
            let tag = gtk4::TextTag::new(Some(name));
            tag.set_property("weight", if *bold { 700 } else { 400 });
            tag.set_property(
                "style",
                if *italic {
                    gtk4::pango::Style::Italic
                } else {
                    gtk4::pango::Style::Normal
                },
            );
            t.add(&tag);
        }
    }
    retint(buf);
}

/// Re-colours existing tags for the current light/dark scheme —
/// called on theme switches so highlighting stays readable.
pub fn retint(buf: &gtk4::TextBuffer) {
    let dark = libadwaita::StyleManager::default().is_dark();
    let t = buf.tag_table();
    for (name, light, darkfg, _, _) in PALETTE {
        if let Some(tag) = t.lookup(name) {
            tag.set_property("foreground", if dark { *darkfg } else { *light });
        }
    }
}

/// Markdown pass — dedicated because prose must not get generic
/// number/string tagging. Handles fences, headers, quotes, lists
/// and the inline spans (bold/italic/code/links).
fn apply_md(buf: &gtk4::TextBuffer) {
    ensure_tags(buf);
    let (s, e) = buf.bounds();
    let text = buf.text(&s, &e, true).to_string();
    let bytes = text.as_bytes();
    let mut base = 0i32;
    let mut ls = 0usize;
    let mut fence = false;
    while ls <= bytes.len() {
        let le = text[ls..].find('\n').map(|i| ls + i).unwrap_or(bytes.len());
        let line = &text[ls..le];
        let t = line.trim_start();
        if t.starts_with("```") {
            put(buf, base, line, 0, bytes_of(line), "cde");
            fence = !fence;
        } else if fence {
            put(buf, base, line, 0, bytes_of(line), "cde");
        } else if t.starts_with('#')
            && t.chars().take_while(|c| *c == '#').count() <= 6
            && t.chars().nth(t.chars().take_while(|c| *c == '#').count()) == Some(' ')
        {
            put(buf, base, line, line.len() - t.len(), bytes_of(line), "hdr");
        } else if t.starts_with('>') {
            put(buf, base, line, line.len() - t.len(), bytes_of(line), "qte");
        } else {
            if let Some(p) = list_marker(t) {
                put(
                    buf,
                    base,
                    line,
                    line.len() - t.len(),
                    line.len() - t.len() + p,
                    "kw",
                );
            }
            md_inline(buf, base, line);
        }
        if le == bytes.len() {
            break;
        }
        base += text[ls..=le].chars().count() as i32;
        ls = le + 1;
    }
}

/// Byte length helper — `line` is a &str slice already.
fn bytes_of(line: &str) -> usize {
    line.len()
}

/// Length of a leading list marker ("- ", "* ", "+ ", "1. ", "12) ").
fn list_marker(t: &str) -> Option<usize> {
    let b = t.as_bytes();
    if b.len() >= 2 && (b[0] == b'-' || b[0] == b'*' || b[0] == b'+') && b[1] == b' ' {
        return Some(2);
    }
    let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if d >= 1 && d <= 3 && b.get(d) == Some(&b'.') && b.get(d + 1) == Some(&b' ') {
        return Some(d + 2);
    }
    None
}

/// Inline markdown spans: `code`, **bold**, *italic*, [link](url),
/// bare http(s) URLs.
fn md_inline(buf: &gtk4::TextBuffer, base: i32, line: &str) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'`' => {
                if let Some(j) = line[i + 1..].find('`') {
                    put(buf, base, line, i, i + j + 2, "cde");
                    i += j + 2;
                } else {
                    i += 1;
                }
            }
            b'*' | b'_' if i + 1 < b.len() && b[i + 1] == b[i] => {
                let pat = &line[i..i + 2];
                if let Some(j) = line[i + 2..].find(pat) {
                    put(buf, base, line, i, i + j + 4, "bld");
                    i += j + 4;
                } else {
                    i += 1;
                }
            }
            b'*' | b'_' => {
                if let Some(j) = line[i + 1..].find(b[i] as char) {
                    if j > 0 {
                        put(buf, base, line, i, i + j + 2, "itl");
                        i += j + 2;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            b'[' => {
                if let Some(j) = line[i..].find("](") {
                    let end = line[i + j + 2..]
                        .find(')')
                        .map(|k| i + j + 3 + k)
                        .unwrap_or(i + j + 2);
                    put(buf, base, line, i, end, "lnk");
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'h' if line[i..].starts_with("http") => {
                let mut j = i;
                while j < b.len() && !b[j].is_ascii_whitespace() {
                    j += 1;
                }
                put(buf, base, line, i, j, "lnk");
                i = j;
            }
            _ => i += 1,
        }
    }
}

/// Re-highlights the whole buffer — one pass over each line.
/// `base` is the char offset where `text` begins inside the buffer.
pub fn apply(buf: &gtk4::TextBuffer, lang: Lang) {
    if lang == Lang::Plain {
        return;
    }
    if lang == Lang::Md {
        apply_md(buf);
        return;
    }
    ensure_tags(buf);
    let (s, e) = buf.bounds();
    let text = buf.text(&s, &e, true).to_string();
    let (cmt, quotes) = line_rules(lang);
    let kws = keywords(lang);
    let mut base_char = 0i32; // running char offset of the current line
    let bytes = text.as_bytes();
    let mut ls = 0usize;
    while ls <= bytes.len() {
        let le = text[ls..].find('\n').map(|i| ls + i).unwrap_or(bytes.len());
        tag_line(buf, base_char, &text[ls..le], cmt, quotes, kws, lang);
        if le == bytes.len() {
            break;
        }
        base_char += text[ls..=le].chars().count() as i32;
        ls = le + 1;
    }
}

/// Returns (line-comment marker, string quotes) for a language.
fn line_rules(l: Lang) -> (Option<&'static str>, &'static [char]) {
    match l {
        Lang::Cpp | Lang::Js | Lang::Ts | Lang::Rs | Lang::Css | Lang::Php | Lang::Sql => {
            (Some("//"), &['"', '\''])
        }
        Lang::Python | Lang::Bash | Lang::Yaml | Lang::Ini | Lang::Docker | Lang::Cmake => {
            (Some("#"), &['"', '\''])
        }
        Lang::Json => (None, &['"']),
        Lang::Xml | Lang::Html => (None, &['"', '\'']),
        _ => (None, &['"', '\'']),
    }
}

fn keywords(l: Lang) -> &'static [&'static str] {
    match l {
        Lang::Cpp => CPP,
        Lang::Python => PY,
        Lang::Js | Lang::Ts => JS,
        Lang::Rs => RS,
        Lang::Bash => SH,
        Lang::Sql => SQL,
        Lang::Php => PHP,
        Lang::Cmake | Lang::Docker => CML,
        _ => &[],
    }
}

/// Tags one logical line: strings first, then numbers/keywords only
/// up to the comment marker, comments last (full remainder).
fn tag_line(
    buf: &gtk4::TextBuffer,
    base: i32,
    line: &str,
    cmt: Option<&str>,
    quotes: &[char],
    kws: &[&str],
    lang: Lang,
) {
    let bytes = line.as_bytes();
    let cstart = cmt.and_then(|c| line.find(c)).unwrap_or(bytes.len());
    let mut i = 0usize;
    while i < cstart {
        let b = bytes[i];
        if quotes.contains(&(b as char)) {
            let mut j = i + 1;
            while j < cstart && bytes[j] != b && bytes[j - 1] != b'\\' {
                j += 1;
            }
            put(buf, base, line, i, j.min(cstart), "str");
            i = j;
            continue;
        }
        if b.is_ascii_digit() && (i == 0 || !is_ident(bytes[i - 1])) {
            let mut j = i + 1;
            while j < cstart
                && (bytes[j].is_ascii_alphanumeric() || b".xX_-".contains(&bytes[j].into()))
            {
                j += 1;
            }
            put(buf, base, line, i, j, "num");
            i = j;
            continue;
        }
        if is_ident(b) {
            let mut j = i + 1;
            while j < cstart && is_ident(bytes[j]) {
                j += 1;
            }
            if kws.contains(&&line[i..j]) {
                put(buf, base, line, i, j, "kw");
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if cstart < bytes.len() {
        put(buf, base, line, cstart, bytes.len(), "cmt");
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with("#include") || trimmed.starts_with("#define") {
        put(
            buf,
            base,
            line,
            line.len() - trimmed.len(),
            bytes.len(),
            "hdr",
        );
    }
    if lang == Lang::Md && trimmed.starts_with('#') {
        put(
            buf,
            base,
            line,
            line.len() - trimmed.len(),
            bytes.len(),
            "hdr",
        );
    }
}

/// Applies a named tag to the byte range [a,b) of `line` starting at
/// char offset `base` inside the buffer.
fn put(buf: &gtk4::TextBuffer, base: i32, line: &str, a: usize, b: usize, name: &str) {
    if a >= b || a >= line.len() {
        return;
    }
    let b = b.min(line.len());
    let ca = base + line[..a].chars().count() as i32;
    let cb = ca + line[a..b].chars().count() as i32;
    let mut it = buf.iter_at_offset(ca);
    let jt = buf.iter_at_offset(cb);
    if let Some(tag) = buf.tag_table().lookup(name) {
        buf.apply_tag(&tag, &mut it, &jt);
    }
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}
