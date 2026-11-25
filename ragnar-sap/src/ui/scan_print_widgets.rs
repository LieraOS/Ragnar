// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — scan-window widget helpers
//
// Small builders shared by the scan-and-print window layout and the
// value mappers helpers provide (the color-mode mapper,
// the DPI parser, source item-data handling).

use gtk4::prelude::*;

use crate::i18n::tr;

/// DropDown + its replaceable StringList model — capability combos
/// get rebuilt whenever the scanner selection changes.
pub struct UiCombo {
 pub dd: gtk4::DropDown,
 pub model: gtk4::StringList,
}

impl UiCombo {
 /// Build a combo from initial items.
 pub fn new<S: AsRef<str>>(items: &[S]) -> Self {
 let strings: Vec<&str> = items.iter().map(|s| s.as_ref()).collect();
 let model = gtk4::StringList::new(&strings);
 let dd = gtk4::DropDown::new(Some(model.clone()), gtk4::Expression::NONE);
 dd.set_hexpand(true);
 Self { dd, model }
 }

 /// Rebuild the item list and pick `keep` (clamped).
 pub fn set_items<S: AsRef<str>>(&self, items: &[S], keep: u32) {
 while self.model.n_items() > 0 {
 self.model.remove(self.model.n_items() - 1);
 }
 for it in items {
 self.model.append(it.as_ref());
 }
 self.dd.set_selected(keep.min(self.model.n_items().saturating_sub(1)));
 }

 /// Selected item text ("" when empty).
 pub fn text(&self) -> String {
 self.dd
.selected_item()
.and_downcast::<gtk4::StringObject>()
.map(|s| s.string().to_string())
.unwrap_or_default()
 }
}

/// Share a UiCombo in an Rc (widgets are cheap GObjects).
pub fn share(cb: &UiCombo) -> Rc<UiCombo> {
 Rc::new(UiCombo { dd: cb.dd.clone(), model: cb.model.clone() })
}

use std::rc::Rc;

/// GroupBox equivalent: titled frame around a vertical box.
pub fn group_box(title: &str) -> gtk4::Box {
 let frame = gtk4::Frame::new(Some(title));
 let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
 inner.set_margin_top(6);
 inner.set_margin_bottom(8);
 inner.set_margin_start(8);
 inner.set_margin_end(8);
 frame.set_child(Some(&inner));
 let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
 outer.append(&frame);
 outer
}

/// The *inner* box of a group (children go here, not the frame).
pub fn group_inner(group: &gtk4::Box) -> gtk4::Box {
 group.first_child()
.and_then(|f| f.downcast::<gtk4::Frame>().ok())
.and_then(|f| f.child())
.and_downcast::<gtk4::Box>()
.unwrap_or_else(|| group.clone())
}

/// Label + widget row used inside the groups.
pub fn row(label: &str, widget: &impl IsA<gtk4::Widget>) -> gtk4::Box {
 let r = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
 let l = gtk4::Label::builder().label(label).xalign(0.0).width_chars(12).build();
 r.append(&l);
 widget.set_hexpand(true);
 r.append(widget);
 r
}

/// Pre-checked CheckButton.
pub fn checked(label: &str) -> gtk4::CheckButton {
 let b = gtk4::CheckButton::with_label(label);
 b.set_active(true);
 b
}

/// Horizontal Scale with range + default value.
pub fn slider(min: f64, max: f64, val: f64) -> gtk4::Scale {
 let s = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, 1.0);
 s.set_value(val);
 s.set_hexpand(true);
 s
}

/// Select the first combo item containing `needle` — semantic match
/// like the semantic matcher because real SANE mode strings differ
/// per device ("24bit Color", "True Gray",.).
pub fn select_containing(cb: &UiCombo, needle: &str) {
 let needle_l = needle.to_lowercase();
 for i in 0..cb.model.n_items() {
 if let Some(item) = cb.model.item(i).and_downcast::<gtk4::StringObject>() {
 if item.string().to_lowercase().contains(&needle_l) {
 cb.dd.set_selected(i);
 return;
 }
 }
 }
}

/// Like select_containing but reports whether anything matched.
fn try_select(cb: &UiCombo, needle: &str) -> bool {
 let n = needle.to_lowercase();
 for i in 0..cb.model.n_items() {
 if let Some(item) = cb.model.item(i).and_downcast::<gtk4::StringObject>() {
 if item.string().to_lowercase().contains(&n) {
 cb.dd.set_selected(i);
 return true;
 }
 }
 }
 false
}

/// Select a combo item for a stored SANE mode value — literal match
/// first (covers values stored by ragnar-settings), then semantic
/// synonyms because devices name modes differently ("Black & White"
/// vs "Lineart", "24bit Color" vs "Color").
pub fn select_mode(cb: &UiCombo, stored: &str) {
 if stored.is_empty() || try_select(cb, stored) {
 return;
 }
 let l = stored.to_lowercase();
 let terms: &[&str] = if l.contains("line") || l.contains("black") || l.contains("schwarz") {
 &["lineart", "black", "b&w", "schwarz", "binary"]
 } else if l.contains("gra") {
 &["gray", "grey", "grau", "monochrom"]
 } else {
 &["color", "farb", "rgb", "24bit"]
 };
 for t in terms {
 if try_select(cb, t) {
 return;
 }
 }
}

/// Select a combo item for a stored SANE source value — ADF variants
/// by keyword; flatbed/platen/glass maps to the first item (the
/// localized "scanner glass" label is always index 0).
pub fn select_source(cb: &UiCombo, stored: &str) {
 let l = stored.to_lowercase();
 if l.contains("duplex") {
 try_select(cb, "duplex");
 } else if l.contains("adf") || l.contains("feeder") {
 try_select(cb, "adf");
 } else {
 cb.dd.set_selected(0);
 }
}

/// UI label → SANE mode string.
pub fn map_color_mode(label: &str) -> String {
 let l = label.to_lowercase();
 if l.contains("gray") || l.contains(&tr("grau").to_lowercase()) {
 "Gray".into()
 } else if l.contains("black") || l.contains("lineart")
 || l.contains(&tr("schwarz").to_lowercase()) {
 "Lineart".into()
 } else {
 "Color".into()
 }
}

/// "300 DPI" → 300 (, fallback 300).
pub fn parse_dpi(label: &str) -> i32 {
 label.split_whitespace().next().and_then(|d| d.parse().ok()).unwrap_or(300)
}

/// UI source label → SANE source string. The stored the verbatim
/// SANE string in item data; our combo shows canonical names.
pub fn source_value(label: &str) -> String {
 let l = label.to_lowercase();
 if l.contains("duplex") {
 "ADF Duplex".into()
 } else if l.contains("adf") {
 "ADF".into()
 } else {
 "Flatbed".into()
 }
}
