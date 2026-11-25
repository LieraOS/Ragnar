// Copyright (C) Jörg Schröder
//! One desktop surface per output. Every monitor carries only the
//! icons assigned to it: the shared layout map (conn → positions)
//! decides where an item lives. The primary output claims whatever
//! no connected monitor owns. Rebuilt on monitor hotplug.

use crate::desktop::{main_monitor, Surface};
use crate::layout::{self, Positions};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

type Shared = Rc<RefCell<BTreeMap<String, Positions>>>;
type List = Rc<RefCell<Vec<Rc<Surface>>>>;

/// Keeps one `Surface` per connected monitor for the app lifetime;
/// rebuilt whenever the output set changes.
pub fn watch(app: &libadwaita::Application) {
    let shared: Shared = Rc::new(RefCell::new(layout::load_all()));
    let primary = main_monitor().and_then(|m| m.connector().map(|c| c.to_string()));
    // The pre-multi-monitor layout file (key "") belongs to the
    // primary output's section.
    {
        let mut m = shared.borrow_mut();
        if let (Some(p), Some(c)) = (m.remove(""), primary.clone()) {
            m.insert(c, p);
        }
    }
    // One id, one monitor: earlier versions wrote every item into
    // every monitor's file — strip such leftovers, the primary
    // section wins conflicts.
    {
        let mut all = shared.borrow_mut();
        let mut order: Vec<String> = primary.iter().cloned().collect();
        order.extend(all.keys().filter(|k| Some(*k) != primary.as_ref()).cloned());
        let mut seen = std::collections::HashSet::new();
        for c in order {
            let Some(p) = all.get_mut(&c) else { continue };
            let mut dirty = false;
            p.retain(|id, _| {
                let keep = seen.insert(id.clone());
                dirty |= !keep;
                keep
            });
            if dirty {
                layout::save(&c, p);
            }
        }
    }
    let list: List = Rc::new(RefCell::new(Vec::new()));
    rebuild(app, &list, &shared);
    if let Some(disp) = gtk4::gdk::Display::default() {
        let monitors = disp.monitors();
        let (app, list, shared) = (app.clone(), list.clone(), shared.clone());
        monitors.connect_items_changed(move |_, _, _, _| rebuild(&app, &list, &shared));
    }
    std::mem::forget((list, shared));
}

/// Recreate the surface set: close the old windows (their outputs
/// may be gone) and build one surface per connected monitor. The
/// shared layout survives the rebuild — positions stay put.
fn rebuild(app: &libadwaita::Application, list: &List, shared: &Shared) {
    for s in list.borrow().iter() {
        s.window.close();
    }
    list.borrow_mut().clear();
    let Some(disp) = gtk4::gdk::Display::default() else {
        return;
    };
    let primary = main_monitor().and_then(|m| m.connector().map(|c| c.to_string()));
    for i in 0..disp.monitors().n_items() {
        let Some(mon) = disp.monitors().item(i).and_downcast::<gtk4::gdk::Monitor>() else {
            continue;
        };
        let s = Surface::new(
            app,
            &mon,
            mon.connector().map(|c| c.to_string()) == primary,
            shared.clone(),
            Rc::downgrade(list),
        );
        s.window.present();
        list.borrow_mut().push(s);
    }
    // Deferred so every surface is already in the peers list when
    // membership is decided.
    for s in list.borrow().iter() {
        s.refresh();
    }
}
