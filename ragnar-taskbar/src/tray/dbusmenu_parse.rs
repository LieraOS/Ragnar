// Copyright (C) Jörg Schröder
// com.canonical.dbusmenu layout parsing. GetLayout returns nested
// (id, props, children) tuples — this module turns that variant tree
// into typed MenuItems the renderer walks recursively.
use gtk4::glib;

/// One menu entry with every property the renderer needs.
#[derive(Clone, Default)]
pub struct MenuItem {
    /// Item id — goes back to the service as the Event target.
    pub id: i32,
    /// Display label (mnemonic underscores stripped).
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    /// "standard" | "separator" — others render as standard.
    pub kind: String,
    /// freedesktop icon name, when the service ships one.
    pub icon_name: String,
    /// Raw icon bytes (PNG or ARGB32) from icon-data.
    pub icon_data: Option<Vec<u8>>,
    /// "" | "checkmark" | "radio" — toggle item type.
    pub toggle_type: String,
    /// 0/1 — current toggle state.
    pub toggle_state: i32,
    /// children-display == "submenu".
    pub has_submenu: bool,
    /// Nested entries — populated for submenu items.
    pub children: Vec<MenuItem>,
}

/// Parse a GetLayout layout tuple into the item list the root box
/// renders (children of the root node).
pub fn parse_tree(layout: &glib::Variant) -> Vec<MenuItem> {
    parse_children(&layout.child_value(2))
}

/// Parse one (i,a{sv},av) tuple — used for the root and recursion.
fn parse_node(node: &glib::Variant) -> MenuItem {
    let mut item = MenuItem {
        enabled: true,
        visible: true,
        ..Default::default()
    };
    item.id = node.child_value(0).get::<i32>().unwrap_or(0);
    let props = node.child_value(1);
    for j in 0..props.n_children() {
        let kv = props.child_value(j);
        let key = kv.child_value(0).str().unwrap_or("").to_string();
        // a{sv} values are variant-wrapped — unwrap once.
        let vv = kv.child_value(1);
        let val = if vv.type_() == glib::VariantTy::VARIANT {
            vv.child_value(0)
        } else {
            vv
        };
        match key.as_str() {
            "label" => {
                if let Some(s) = val.get::<String>() {
                    // '_' marks the mnemonic key, not literal text.
                    item.label = s.replace('_', "");
                }
            }
            "enabled" => item.enabled = val.get::<bool>().unwrap_or(true),
            "visible" => item.visible = val.get::<bool>().unwrap_or(true),
            "type" => item.kind = val.get::<String>().unwrap_or_default(),
            "icon-name" => item.icon_name = val.get::<String>().unwrap_or_default(),
            "icon-data" => item.icon_data = val.get::<Vec<u8>>(),
            "toggle-type" => item.toggle_type = val.get::<String>().unwrap_or_default(),
            "toggle-state" => item.toggle_state = val.get::<i32>().unwrap_or(0),
            "children-display" => {
                item.has_submenu = val.get::<String>().as_deref() == Some("submenu")
            }
            _ => {}
        }
    }
    item.children = parse_children(&node.child_value(2));
    item
}

/// The `av` children array — each entry is a variant-wrapped tuple.
fn parse_children(children: &glib::Variant) -> Vec<MenuItem> {
    let mut out = Vec::new();
    for i in 0..children.n_children() {
        let item = children.child_value(i);
        let inner = if item.type_() == glib::VariantTy::VARIANT {
            item.child_value(0)
        } else {
            item
        };
        out.push(parse_node(&inner));
    }
    out
}
