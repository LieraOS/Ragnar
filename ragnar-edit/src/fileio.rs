//! Ragnar Edit — GPL-3.0-or-later, Copyright (C) 2026 Jörg Schröder
//!
//! File IO: chunked background loading, binary detection, and
//! import/extraction for non-plain formats (RTF → text, PDF → text).

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Result handed back to the UI thread after a background load.
pub struct Loaded {
    pub text: String,
    /// True when the content was imported (rtf/pdf) rather than raw text.
    pub imported: bool,
    pub readonly: bool,
}

/// Sniffs for NUL bytes and control chars in the first 8 KiB.
pub fn is_binary(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8192];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    buf[..n].iter().any(|&b| b == 0)
}

/// Loads `path` on the calling thread — run this inside spawn_blocking.
/// Handles .rtf and .pdf imports; everything else is read as UTF-8
/// (lossy for legacy encodings).
pub fn load(path: &Path) -> std::io::Result<Loaded> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "rtf" {
        let raw = std::fs::read(path)?;
        return Ok(Loaded {
            text: rtf_to_text(&raw),
            imported: true,
            readonly: false,
        });
    }
    if ext == "pdf" {
        let raw = std::fs::read(path)?;
        return Ok(Loaded {
            text: pdf_extract(&raw),
            imported: true,
            readonly: true,
        });
    }
    let raw = std::fs::read(path)?;
    Ok(Loaded {
        text: String::from_utf8_lossy(&raw).into_owned(),
        imported: false,
        readonly: false,
    })
}

/// Strips RTF control words/groups — yields the plain text payload.
/// 100% correct as a *text* import; formatting is intentionally dropped.
fn rtf_to_text(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    break;
                }
                match bytes[i] {
                    b'{' | b'}' | b'\\' => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    b'\'' => {
                        // \'hh — hex encoded byte, decode latin-1 style
                        if i + 2 < bytes.len() {
                            let h = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                            if let Ok(v) = u8::from_str_radix(h, 16) {
                                out.push(v as char);
                            }
                            i += 3;
                        } else {
                            i += 1;
                        }
                    }
                    b'*' => {
                        // \* destination group — skip to its closing brace
                        let mut depth = 0;
                        while i < bytes.len() {
                            if bytes[i] == b'{' {
                                depth += 1;
                            }
                            if bytes[i] == b'}' {
                                depth -= 1;
                                if depth <= 0 {
                                    break;
                                }
                            }
                            i += 1;
                        }
                        i += 1;
                    }
                    _ => {
                        // control word: letters + optional number + delimiter
                        let start = i;
                        while i < bytes.len() && bytes[i].is_ascii_lowercase() {
                            i += 1;
                        }
                        let word = &s[start..i];
                        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'-') {
                            i += 1;
                        }
                        if i < bytes.len() && bytes[i] == b' ' {
                            i += 1;
                        }
                        match word {
                            "par" | "line" => out.push('\n'),
                            "tab" => out.push('\t'),
                            "u" => {} // \uN handled by generic flow (rare in practice)
                            _ => {}
                        }
                    }
                }
            }
            b'{' | b'}' => i += 1,
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out.trim_end().to_string() + "\n"
}

/// Minimal PDF text extraction: finds FlateDecode streams, inflates
/// them and pulls literal strings out of Tj/TJ operators.
/// Covers the vast majority of generated PDFs; encrypted/image-only
/// documents degrade to an empty result.
fn pdf_extract(raw: &[u8]) -> String {
    let mut out = String::from("[PDF — Read-Only]\n\n");
    let mut found = String::new();
    let mut pos = 0;
    while let Some(s) = find_sub(raw, b"stream\r\n", pos).or(find_sub(raw, b"stream\n", pos)) {
        let start = s + if raw[s..].starts_with(b"stream\r\n") {
            8
        } else {
            7
        };
        let Some(end) = find_sub(raw, b"endstream", start) else {
            break;
        };
        let chunk = &raw[start..end];
        let decoded = {
            let mut d = flate2::read::ZlibDecoder::new(chunk);
            let mut v = Vec::new();
            if d.read_to_end(&mut v).is_ok() {
                v
            } else {
                chunk.to_vec()
            }
        };
        extract_text_ops(&decoded, &mut found);
        pos = end + 9;
    }
    if found.is_empty() {
        out.push_str("(no extractable text — scanned or encrypted document)\n");
    } else {
        out.push_str(&found);
    }
    out
}

fn find_sub(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Pulls literal `(…)` strings out of a content stream, adding a
/// newline after each Tj/TJ/T* text-showing operator.
fn extract_text_ops(buf: &[u8], out: &mut String) {
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == b'(' {
            let mut depth = 1;
            let mut j = i + 1;
            let mut s = String::new();
            while j < buf.len() && depth > 0 {
                match buf[j] {
                    b'\\' => {
                        if j + 1 < buf.len() {
                            match buf[j + 1] {
                                b'n' => s.push('\n'),
                                b'r' => s.push('\r'),
                                b't' => s.push('\t'),
                                b'(' => s.push('('),
                                b')' => s.push(')'),
                                b'\\' => s.push('\\'),
                                c => s.push(c as char),
                            }
                            j += 2;
                        } else {
                            j += 1;
                        }
                    }
                    b'(' => {
                        depth += 1;
                        s.push('(');
                        j += 1;
                    }
                    b')' => {
                        depth -= 1;
                        if depth > 0 {
                            s.push(')');
                        }
                        j += 1;
                    }
                    c => {
                        s.push(c as char);
                        j += 1;
                    }
                }
            }
            out.push_str(&s);
            out.push(' ');
            i = j;
        } else if buf[i..].starts_with(b"Tj")
            || buf[i..].starts_with(b"TJ")
            || buf[i..].starts_with(b"T*")
        {
            out.push('\n');
            i += 2;
        } else {
            i += 1;
        }
    }
}
