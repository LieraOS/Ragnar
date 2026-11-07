//! The hub overview: category groups with tile grids.
//!
//! Built from the registry catalog; tiles whose requirements are not
//! met by the host are skipped entirely (not merely greyed out).
//! Returns the page content; scrolling is owned by the caller so the
//! page can be rebuilt on search without losing the search field's
//! focus.

use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::WrapBox;
use gtk4::{Box, Label, Orientation};

use crate::backend::satisfied;
use crate::registry::{self, search, Entry};

/// Build the hub page content for `query` (empty for the full view).
///
/// `on_entry` is invoked with the activated [`Entry`]. Tiles are
/// filtered through [`search::matches`].
pub fn build<F>(query: &str, on_entry: Rc<F>) -> Box
where
    F: Fn(&'static Entry) + 'static + ?Sized,
{
    let page = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    for category in registry::categories() {
        let entries: Vec<_> = registry::entries_in(category.id)
            .into_iter()
            .filter(|e| satisfied(&e.requirement))
            .filter(|e| search::matches(e, query))
            .collect();
        if entries.is_empty() {
            continue;
        }
        let header = Label::builder()
            .label(gettextrs::gettext(category.title))
            .xalign(0.0)
            .build();
        header.add_css_class("title-3");
        page.append(&header);
        page.append(&grid_for(&entries, Rc::clone(&on_entry)));
    }
    page
}

/// A wrapped layout of tile buttons for one category.
///
/// `AdwWrapBox` is used instead of `GtkFlowBox`: flow box children have
/// their own focus and selection semantics which swallow the first
/// click after a navigation transition.
fn grid_for<F>(entries: &[&'static Entry], on_entry: Rc<F>) -> WrapBox
where
    F: Fn(&'static Entry) + 'static + ?Sized,
{
    let grid = WrapBox::builder()
        .child_spacing(12)
        .line_spacing(12)
        .line_homogeneous(true)
        .build();

    for entry in entries {
        let entry = *entry;
        let on_entry = Rc::clone(&on_entry);
        let tile = super::tile::build(entry, move || on_entry(entry));
        tile.set_size_request(240, -1);
        tile.set_hexpand(false);
        grid.append(&tile);
    }
    grid
}
