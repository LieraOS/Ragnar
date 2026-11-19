// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — window-rule model: parse, edit, serialize
// Copyright (C) 2026 LieraOS Project
//
// Text-level model of `<windowRules>` in the compositor's rc.xml.
// Each rule keeps its attribute list in original order and its inner
// XML verbatim — attributes the editor does not model, comments and
// unknown actions survive an edit untouched. Rewrites replace exactly
// the edited rule's text span; unrelated content is never rebuilt.

/// One `<windowRule>`: attributes in source order, inner XML verbatim.
#[derive(Debug, Clone)]
pub struct Rule {
    pub attrs: Vec<(String, String)>,
    pub inner: String,
}

impl Rule {
    /// Read an attribute value.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Set an attribute, keeping the original position; removes it
    /// when `val` is None.
    pub fn set_attr(&mut self, name: &str, val: Option<&str>) {
        match val {
            Some(v) => {
                if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| k == name) {
                    slot.1 = v.to_string();
                } else {
                    self.attrs.push((name.to_string(), v.to_string()));
                }
            }
            None => self.attrs.retain(|(k, _)| k != name),
        }
    }

    /// Whether an `<action name="N">` element exists inside the rule.
    pub fn has_action(&self, name: &str) -> bool {
        self.inner.contains(&format!("name=\"{name}\""))
    }

    /// Read an attribute of the first `<action name="N">` element.
    pub fn action_attr(&self, name: &str, attr: &str) -> Option<String> {
        let a = self.inner.find(&format!("name=\"{name}\""))?;
        let tail = &self.inner[a..];
        let end = tail.find("/>").or_else(|| tail.find("</action>"))?;
        let elem = &tail[..end];
        let pat = format!("{attr}=\"");
        let p = elem.find(&pat)? + pat.len();
        let e = elem[p..].find('"')?;
        Some(elem[p..p + e].to_string())
    }

    /// Append an action element inside the rule.
    pub fn add_action(&mut self, xml: &str) {
        self.inner.push_str(&format!("\n\t\t\t{xml}"));
    }

    /// Remove every `<action name="N">` element — self-closing or
    /// with a body — leaving other actions and comments intact.
    pub fn remove_action(&mut self, name: &str) {
        loop {
            let Some(name_at) = self.inner.find(&format!("name=\"{name}\"")) else {
                return;
            };
            // The <action tag begins before the name attribute.
            let Some(start) = self.inner[..name_at].rfind("<action") else {
                return;
            };
            let after_name = &self.inner[name_at..];
            let sc = after_name.find("/>").map(|i| name_at + i + 2);
            let cl = after_name
                .find("</action>")
                .map(|i| name_at + i + "</action>".len());
            let Some(end) = [sc, cl].into_iter().flatten().min() else {
                return;
            };
            self.inner.replace_range(start..end, "");
        }
    }
}

/// Parse every `<windowRule>` inside `<windowRules>` — returns the
/// rule plus its byte span (incl. tags) for surgical replacement.
pub fn parse(text: &str) -> Vec<(Rule, usize, usize)> {
    let mut out = Vec::new();
    let Some(wr_start) = text.find("<windowRules>") else {
        return out;
    };
    let Some(wr_end) = text[wr_start..]
        .find("</windowRules>")
        .map(|i| wr_start + i)
    else {
        return out;
    };
    let mut pos = wr_start + "<windowRules>".len();
    while let Some(rel) = text[pos..wr_end].find("<windowRule") {
        let start = pos + rel;
        // Skip a closing tag if we ever land on one.
        if text[start..].starts_with("</") {
            pos = start + 2;
            continue;
        }
        let Some(gt) = text[start..wr_end].find('>').map(|i| start + i) else {
            break;
        };
        let tag = &text[start..gt];
        if tag.trim_end().ends_with('/') {
            out.push((
                Rule {
                    attrs: parse_attrs(tag),
                    inner: String::new(),
                },
                start,
                gt + 1,
            ));
            pos = gt + 1;
            continue;
        }
        let Some(close) = text[gt..wr_end].find("</windowRule>").map(|i| gt + i) else {
            break;
        };
        out.push((
            Rule {
                attrs: parse_attrs(tag),
                inner: text[gt + 1..close].to_string(),
            },
            start,
            close + "</windowRule>".len(),
        ));
        pos = close + "</windowRule>".len();
    }
    out
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

/// Serialize a rule at two-tab depth inside <windowRules>.
pub fn serialize(rule: &Rule) -> String {
    let attrs: String = rule
        .attrs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{v}\""))
        .collect();
    if rule.inner.trim().is_empty() {
        format!("\t\t<windowRule{attrs} />\n")
    } else {
        format!("\t\t<windowRule{attrs}>{}\n\t\t</windowRule>\n", rule.inner)
    }
}

/// Replace one parsed rule (by index) with `rule` and write back.
pub fn save_edit(text: &str, idx: usize, rule: &Rule) -> String {
    let rules = parse(text);
    let Some((a, b)) = rules.get(idx).map(|(_, a, b)| (*a, *b)) else {
        return text.to_string();
    };
    format!("{}{}{}", &text[..a], serialize(rule), &text[b..])
}

/// Append a rule before `</windowRules>` — creating the whole block
/// before `</labwc_config>` when absent — and write back.
pub fn save_add(text: &str, rule: &Rule) -> String {
    let ser = serialize(rule);
    if let Some(end) = text.find("</windowRules>") {
        format!("{}{}{}", &text[..end], ser, &text[end..])
    } else if let Some(end) = text.find("</labwc_config>") {
        format!(
            "{}\t<windowRules>\n{}\t</windowRules>\n{}",
            &text[..end],
            ser,
            &text[end..]
        )
    } else {
        text.to_string()
    }
}

/// Remove the rule at index `idx` (its span, nothing else).
pub fn save_delete(text: &str, idx: usize) -> String {
    let rules = parse(text);
    let Some((a, b)) = rules.get(idx).map(|(_, a, b)| (*a, *b)) else {
        return text.to_string();
    };
    format!("{}{}", &text[..a], &text[b..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "<labwc_config>\n\
        \t<windowRules>\n\
        \t\t<!-- PiP stays on top -->\n\
        \t\t<windowRule title=\"Picture-in-Picture\" allowAlwaysOnTop=\"yes\">\n\
        \t\t\t<action name=\"ToggleAlwaysOnTop\" />\n\
        \t\t</windowRule>\n\
        \t\t<windowRule identifier=\"steam_app_*\">\n\
        \t\t\t<action name=\"Focus\" />\n\
        \t\t\t<action name=\"Raise\" />\n\
        \t\t</windowRule>\n\
        \t\t<windowRule identifier=\"foo\" skipTaskbar=\"yes\" />\n\
        \t</windowRules>\n\
        </labwc_config>\n";

    #[test]
    fn windowrules_roundtrip() {
        let rules = parse(DOC);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].0.attr("title"), Some("Picture-in-Picture"));
        assert!(rules[0].0.has_action("ToggleAlwaysOnTop"));
        assert_eq!(rules[2].0.attr("skipTaskbar"), Some("yes"));

        // Edit attrs on rule 0; the comment and other rules survive.
        let mut r = rules[0].0.clone();
        r.set_attr("serverDecoration", Some("no"));
        let t = save_edit(DOC, 0, &r);
        assert!(t.contains("<!-- PiP stays on top -->"));
        assert!(t.contains("identifier=\"steam_app_*\""));
        assert!(t.contains("serverDecoration=\"no\""));
        assert!(t.contains("allowAlwaysOnTop=\"yes\""));

        // Removing the middle action leaves the other one.
        let mut r = rules[1].0.clone();
        r.remove_action("Focus");
        let t = save_edit(DOC, 1, &r);
        assert!(!t.contains("name=\"Focus\""));
        assert!(t.contains("name=\"Raise\""));

        // Delete keeps everything else byte-identical.
        let t = save_delete(DOC, 1);
        assert!(t.contains("Picture-in-Picture"));
        assert!(!t.contains("steam_app"));

        // Append at the end of the block.
        let t = save_add(
            DOC,
            &Rule {
                attrs: vec![("identifier".into(), "bar".into())],
                inner: String::new(),
            },
        );
        let rules = parse(&t);
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[3].0.attr("identifier"), Some("bar"));

        // Missing section is created.
        let bare = "<labwc_config>\n</labwc_config>\n";
        let t = save_add(
            bare,
            &Rule {
                attrs: vec![("identifier".into(), "x".into())],
                inner: String::new(),
            },
        );
        assert!(t.contains("<windowRules>"));
    }
}
