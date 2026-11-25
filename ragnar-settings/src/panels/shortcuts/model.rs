// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — keybind model: parse, edit, serialize
// Copyright (C) 2026 LieraOS Project
//
// Text-level model of <keybind> elements in the compositor's rc.xml.
// Every bind keeps its attribute list and its inner XML verbatim —
// attributes the editor does not model (managed, onRelease, extra
// actions, comments) survive an edit untouched. Rewrites replace
// exactly the edited bind's text span.
//
// Binds carrying managed="labwc-bridge" belong to the session
// bridge — it regenerates them from GSettings, so the UI must
// never write to them.

/// One `<action>` inside a keybind: name plus attributes in order.
#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub attrs: Vec<(String, String)>,
}

impl Action {
    /// Read one action attribute.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// One parsed `<keybind>`.
#[derive(Debug, Clone)]
pub struct Bind {
    /// key="…" combo in labwc notation (e.g. "W-C-Left").
    pub key: String,
    /// Whether the session bridge owns this bind — read-only in UI.
    pub managed: bool,
    /// First action (labwc binds almost always carry exactly one).
    pub action: Action,
    /// Whether further action elements exist beyond the first.
    pub extra_actions: bool,
}

/// Extract `name="value"` pairs from a tag's text in source order.
fn parse_attrs(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let key_start = rest[..eq]
            .rfind(|c: char| c.is_whitespace() || c == '<')
            .map(|i| i + 1)
            .unwrap_or(0);
        let key = rest[key_start..eq].trim();
        let after = &rest[eq + 1..];
        if !after.starts_with('"') || key.is_empty() {
            break;
        }
        let Some(end) = after[1..].find('"') else {
            break;
        };
        out.push((key.to_string(), after[1..1 + end].to_string()));
        rest = &after[1 + end + 1..];
    }
    out
}

/// Parse every `<keybind>` inside `<keyboard>` — returns the bind
/// plus its byte span for surgical replacement.
pub fn parse(text: &str) -> Vec<(Bind, usize, usize)> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut base = 0usize;
    while let Some(rel) = rest.find("<keybind") {
        let start = base + rel;
        let seg = &rest[rel..];
        if seg.starts_with("</") {
            base += rel + 2;
            rest = &rest[rel + 2..];
            continue;
        }
        let Some(gt) = seg.find('>') else { break };
        let tag = &seg[..gt];
        let attrs = parse_attrs(tag);
        let get = |n: &str| attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
        if tag.trim_end().ends_with('/') {
            // <keybind key="…"/> — no actions; still a valid display row.
            if let Some(key) = get("key") {
                out.push((
                    Bind {
                        key,
                        managed: get("managed").as_deref() == Some("labwc-bridge"),
                        action: Action {
                            name: String::new(),
                            attrs: vec![],
                        },
                        extra_actions: false,
                    },
                    start,
                    start + gt + 1,
                ));
            }
            base += rel + gt + 1;
            rest = &seg[gt + 1..];
            continue;
        }
        let Some(close) = seg.find("</keybind>") else {
            break;
        };
        let inner = &seg[gt + 1..close];
        if let Some(key) = get("key") {
            let actions = parse_actions(inner);
            let (first, extra) = match actions.split_first() {
                Some((a, rest_a)) => (a.clone(), !rest_a.is_empty()),
                None => (
                    Action {
                        name: String::new(),
                        attrs: vec![],
                    },
                    false,
                ),
            };
            out.push((
                Bind {
                    key,
                    managed: get("managed").as_deref() == Some("labwc-bridge"),
                    action: first,
                    extra_actions: extra,
                },
                start,
                start + close + "</keybind>".len(),
            ));
        }
        // `close` is already an index into seg — adding gt again
        // would eat the head of the next keybind.
        base += rel + close + "</keybind>".len();
        rest = &seg[close + "</keybind>".len()..];
    }
    out
}

/// Parse all `<action>` elements inside a keybind's inner text.
fn parse_actions(inner: &str) -> Vec<Action> {
    let mut out = Vec::new();
    let mut rest = inner;
    while let Some(pos) = rest.find("<action") {
        let seg = &rest[pos..];
        let Some(gt) = seg.find('>') else { break };
        let tag = &seg[..gt];
        let attrs = parse_attrs(tag);
        let name = attrs
            .iter()
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if !name.is_empty() {
            out.push(Action {
                name,
                attrs: attrs.into_iter().filter(|(k, _)| k != "name").collect(),
            });
        }
        // Skip a body if the action is not self-closing.
        if tag.trim_end().ends_with('/') {
            rest = &seg[gt + 1..];
        } else if let Some(c) = seg.find("</action>") {
            rest = &seg[c + "</action>".len()..];
        } else {
            break;
        }
    }
    out
}

/// Serialize a bind at two-tab depth inside <keyboard>.
pub fn serialize(bind: &Bind, action: &Action) -> String {
    let attrs: String = action
        .attrs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{v}\""))
        .collect();
    format!(
        "\t\t<keybind key=\"{}\"><action name=\"{}\"{attrs} /></keybind>\n",
        bind.key, action.name
    )
}

/// Replace the bind at index `idx` with a new key + action.
pub fn save_edit(text: &str, idx: usize, key: &str, action: &Action) -> String {
    let binds = parse(text);
    let Some((a, b)) = binds.get(idx).map(|(_, a, b)| (*a, *b)) else {
        return text.to_string();
    };
    let ser = serialize(
        &Bind {
            key: key.to_string(),
            managed: false,
            action: action.clone(),
            extra_actions: false,
        },
        action,
    );
    format!("{}{}{}", &text[..a], ser, &text[b..])
}

/// Append a bind before `</keyboard>`.
pub fn save_add(text: &str, key: &str, action: &Action) -> String {
    let ser = serialize(
        &Bind {
            key: key.to_string(),
            managed: false,
            action: action.clone(),
            extra_actions: false,
        },
        action,
    );
    match text.find("</keyboard>") {
        Some(end) => format!("{}{}{}", &text[..end], ser, &text[end..]),
        None => text.to_string(),
    }
}

/// Remove the bind at index `idx` (its span, nothing else).
pub fn save_delete(text: &str, idx: usize) -> String {
    let binds = parse(text);
    let Some((a, b)) = binds.get(idx).map(|(_, a, b)| (*a, *b)) else {
        return text.to_string();
    };
    format!("{}{}", &text[..a], &text[b..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "<labwc_config>\n\
        \t<keyboard>\n\
        \t\t<keybind key=\"XF86AudioNext\" managed=\"labwc-bridge\">\n\
        \t\t\t<action name=\"Execute\" command=\"dbus-send ...\" />\n\
        \t\t</keybind>\n\
        \t\t<keybind key=\"W-Return\"><action name=\"Execute\" command=\"foot\" /></keybind>\n\
        \t\t<keybind key=\"Print\">\n\
        \t\t\t<action name=\"Execute\" command=\"sh -c shot\" />\n\
        \t\t</keybind>\n\
        \t</keyboard>\n\
        </labwc_config>\n";

    #[test]
    fn keybind_roundtrip() {
        let binds = parse(DOC);
        assert_eq!(binds.len(), 3);
        assert!(binds[0].0.managed);
        assert_eq!(binds[0].0.action.name, "Execute");
        assert_eq!(binds[1].0.action.attr("command"), Some("foot"));

        // Editing bind 1 leaves the managed bind untouched.
        let act = Action {
            name: "Close".into(),
            attrs: vec![],
        };
        let t = save_edit(DOC, 1, "W-q", &act);
        assert!(t.contains("managed=\"labwc-bridge\""));
        assert!(t.contains("key=\"W-q\""));
        assert!(t.contains("name=\"Close\""));

        // Delete keeps others.
        let t = save_delete(DOC, 1);
        assert!(!t.contains("W-Return"));
        assert!(t.contains("XF86AudioNext"));

        // Append before </keyboard>.
        let t = save_add(DOC, "W-e", &act);
        let binds = parse(&t);
        assert_eq!(binds.len(), 4);
        assert_eq!(binds[3].0.key, "W-e");
    }
}
