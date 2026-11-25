// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-eye — the left-hand directory tree.
//
// GtkTreeListModel over GFile children (dirs only) + expanders,
// the GTK4-idiomatic filesystem tree. Selecting a directory asks
// the window to repopulate the thumbnail strip; the model lazy-
// loads children on expand.

use gtk4::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;

pub struct Browser {
    pub widget: gtk4::ScrolledWindow,
    #[allow(dead_code)]
    store: gtk4::gio::ListStore,
    #[allow(dead_code)]
    view: gtk4::ListView,
    tree: gtk4::TreeListModel,
}

impl Browser {
    /// `on_dir` is invoked when the user picks a directory.
    pub fn new(root: &std::path::Path, on_dir: Rc<dyn Fn(PathBuf)>) -> Browser {
        let store = gtk4::gio::ListStore::new::<gtk4::gio::File>();
        // autoexpand MUST stay false: with true the model expands
        // every directory recursively — crawling all of / including
        // /proc and mounts, which blocks window::run before present().
        // children_func receives the row's *item* (the GFile), not
        // the TreeListRow — that distinction broke the first version:
        // the downcast always failed, so the tree never had children.
        let tree = gtk4::TreeListModel::new(store.clone(), false, false, |item| {
            let file = item.downcast_ref::<gtk4::gio::File>()?;
            let path = file.path()?;
            let children = gtk4::gio::ListStore::new::<gtk4::gio::File>();
            if let Ok(rd) = std::fs::read_dir(&path) {
                let mut dirs: Vec<PathBuf> = rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_dir()
                            && !p
                                .file_name()
                                .map(|n| n.to_string_lossy().starts_with('.'))
                                .unwrap_or(true)
                    })
                    .collect();
                dirs.sort();
                for d in dirs {
                    children.append(&gtk4::gio::File::for_path(&d));
                }
            }
            Some(children.upcast::<gtk4::gio::ListModel>())
        });
        // Two roots: home directory and filesystem root — the user
        // can navigate anywhere from here.
        store.append(&gtk4::gio::File::for_path(
            std::env::var("HOME").unwrap_or_else(|_| "/".into()),
        ));
        store.append(&gtk4::gio::File::for_path("/"));
        let _ = root;

        // Open both roots immediately — a collapsed pair of rows is
        // not a navigable tree.
        for i in 0..tree.n_items() {
            if let Some(row) = tree.item(i).and_downcast::<gtk4::TreeListRow>() {
                row.set_expanded(true);
            }
        }

        let sel = gtk4::SingleSelection::new(Some(tree.clone().upcast::<gtk4::gio::ListModel>()));
        let factory = gtk4::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let expander = gtk4::TreeExpander::new();
            expander.set_child(Some(&gtk4::Label::new(None)));
            item.downcast_ref::<gtk4::ListItem>()
                .unwrap()
                .set_child(Some(&expander));
        });
        factory.connect_bind(|_, item| {
            let li = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let expander = li.child().and_downcast::<gtk4::TreeExpander>().unwrap();
            let row = li.item().and_downcast::<gtk4::TreeListRow>().unwrap();
            expander.set_list_row(Some(&row));
            let label = expander.child().and_downcast::<gtk4::Label>().unwrap();
            let name = row
                .item()
                .and_downcast::<gtk4::gio::File>()
                .and_then(|f| f.basename().and_then(|b| b.to_str().map(|s| s.to_string())))
                .unwrap_or_default();
            label.set_text(&name);
        });

        let view = gtk4::ListView::new(Some(sel.clone()), Some(factory));
        view.add_css_class("navigation-sidebar");

        // Selection → directory picked.
        let on_dir2 = on_dir.clone();
        sel.connect_selected_item_notify(move |s| {
            if let Some(row) = s.selected_item().and_downcast::<gtk4::TreeListRow>() {
                if let Some(f) = row.item().and_downcast::<gtk4::gio::File>() {
                    if let Some(p) = f.path() {
                        on_dir2(p);
                    }
                }
            }
        });

        let widget = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(180)
            .build();
        widget.set_child(Some(&view));
        Browser {
            widget,
            store,
            view,
            tree,
        }
    }

    /// Expand every ancestor row of `target` so its directory is
    /// visible in the tree — call with the directory of the file
    /// that was opened so the user sees where they are.
    pub fn expand_to(&self, target: &std::path::Path) {
        expand_chain(&self.tree.clone().upcast::<gtk4::gio::ListModel>(), target);
    }
}

/// Recursively expand rows whose path is an ancestor of `target`.
fn expand_chain(model: &gtk4::gio::ListModel, target: &std::path::Path) {
    for i in 0..model.n_items() {
        let Some(row) = model.item(i).and_downcast::<gtk4::TreeListRow>() else {
            continue;
        };
        let Some(path) = row
            .item()
            .and_downcast::<gtk4::gio::File>()
            .and_then(|f| f.path())
        else {
            continue;
        };
        if target.starts_with(&path) {
            row.set_expanded(true);
            if let Some(children) = row.children() {
                expand_chain(&children, target);
            }
        }
    }
}
