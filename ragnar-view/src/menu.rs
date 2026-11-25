// Copyright (C) Jörg Schröder
//! Context menus. Both menus are real GTK popover menus (nested
//! flyout submenus); shared helpers and the desktop-background menu
//! live here, the icon menu in `menu_file`.

use crate::desktop::Surface;
use crate::i18n::tr;
use crate::items;
use gtk4::prelude::*;
use std::rc::Rc;

/// Spawn a command line detached.
pub(crate) fn spawn(cmd: &str) {
    let _ = gtk4::glib::spawn_command_line_async(cmd);
}

/// Register a stateless action `name` on `group`.
pub(crate) fn action(group: &gio::SimpleActionGroup, name: &str, f: impl Fn() + 'static) {
    let a = gio::SimpleAction::new(name, None);
    a.connect_activate(move |_, _| f());
    group.add_action(&a);
}

/// GTK's popover-menu template wraps every page in a GtkScrolledWindow
/// capped at 320 px (`max-content-height`), so menus get scroll arrows
/// no matter how much room the monitor offers. Lift that cap on every
/// scrolled window inside the popover so the menu shows in full.
fn lift_height_cap(w: &gtk4::Widget) {
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(sw) = c.downcast_ref::<gtk4::ScrolledWindow>() {
            sw.set_max_content_height(-1);
        }
        lift_height_cap(&c);
        child = c.next_sibling();
    }
}

/// Popover menu for `menu` whose `ctx.*` actions come from `group`,
/// parented to the desktop window, shown at window position (x, y)
/// and unparented again once closed (deferred, GTK is still busy
/// with it inside the closed handler).
pub(crate) fn show(
    parent: &impl IsA<gtk4::Widget>,
    menu: &gio::Menu,
    group: &gio::SimpleActionGroup,
    (x, y): (f64, f64),
) {
    let pop = gtk4::PopoverMenu::from_model_full(menu, gtk4::PopoverMenuFlags::NESTED);
    lift_height_cap(pop.upcast_ref::<gtk4::Widget>());
    pop.insert_action_group("ctx", Some(group));
    pop.set_has_arrow(false);
    pop.set_halign(gtk4::Align::Start);
    pop.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    pop.set_parent(parent);
    pop.connect_closed(|p| {
        let p = p.clone();
        gtk4::glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

/// Submenu whose entries are radio items of the settings action
/// `desk.<key>` — `(label, value)` pairs.
fn radio_menu(key: &str, entries: &[(&str, &str)]) -> gio::Menu {
    let m = gio::Menu::new();
    for (label, value) in entries {
        m.append(Some(&tr(label)), Some(&format!("desk.{key}::{value}")));
    }
    m
}

/// Right-click on the bare desktop: create, sort, arrange, open
/// locations, links into ragnar-settings. `parent` is the window the
/// click happened on — the primary surface or a menu surface on
/// another monitor.
pub fn background(s: &Rc<Surface>, parent: &impl IsA<gtk4::Widget>, x: f64, y: f64) {
    let group = gio::SimpleActionGroup::new();
    let refresh = {
        let s = s.clone();
        move || s.refresh()
    };
    // New items land where the menu was opened: icon center on the
    // click point, snapped/dodged like a normal drop.
    let spot = (
        x as i32 - crate::layout::ICON_W / 2,
        y as i32 - crate::layout::ICON_H / 2,
    );
    let (s2, r) = (s.clone(), refresh.clone());
    action(&group, "new-folder", move || {
        if let Some(f) = items::new_folder(&tr("New Folder")) {
            s2.drop_icon(f.uri().as_str(), spot);
        }
        r();
    });
    let s3 = s.clone();
    action(&group, "new-doc", move || {
        if let Some(f) = items::new_document(&tr("New Document")) {
            s3.drop_icon(f.uri().as_str(), spot);
        }
        refresh();
    });
    action(&group, "files", || {
        let _ = gio::AppInfo::launch_default_for_uri(
            &items::desktop_dir().uri(),
            None::<&gio::AppLaunchContext>,
        );
    });
    action(&group, "background", || {
        spawn("ragnar-settings --panel appearance")
    });
    action(&group, "display", || {
        spawn("ragnar-settings --panel display")
    });
    action(&group, "system", || spawn("ragnar-settings"));

    let menu = gio::Menu::new();
    let new = gio::Menu::new();
    new.append(Some(&tr("Folder")), Some("ctx.new-folder"));
    new.append(Some(&tr("Empty Document")), Some("ctx.new-doc"));
    menu.append_submenu(Some(&tr("New")), &new);
    // Sort/arrange are backed by GSettings keys — only offered when
    // the schema is installed.
    if s.settings.is_some() {
        menu.append_submenu(
            Some(&tr("Sort by")),
            &radio_menu(
                "sort-by",
                &[
                    ("Name", "name"),
                    ("Modification Date", "modified"),
                    ("Size", "size"),
                    ("Type", "type"),
                ],
            ),
        );
        menu.append_submenu(
            Some(&tr("Icon Arrangement")),
            &radio_menu(
                "arrange",
                &[
                    ("Snap to Grid", "grid"),
                    ("Free Placement", "free"),
                    ("Auto Arrange", "auto"),
                ],
            ),
        );
    }
    // Flat single-section layout on purpose: every GMenu section makes
    // GTK's popover menu measure ~13 px short, which forces scrolling.
    menu.append(Some(&tr("Open in File Manager")), Some("ctx.files"));
    menu.append(Some(&tr("Background")), Some("ctx.background"));
    menu.append(Some(&tr("Display Settings")), Some("ctx.display"));
    menu.append(Some(&tr("System Settings")), Some("ctx.system"));
    show(parent, &menu, &group, (x, y));
}
