//! Right-click menu of a desktop icon: open, open with, terminal,
//! rename, delete. Entries and the delete behaviour follow the
//! user's preferences (ragnar-settings → Panel and desktop).

use crate::desktop::Surface;
use crate::i18n::tr;
use crate::items::{self, Item};
use crate::menu::{action, show, spawn};
use crate::prefs::{DeleteAction, Prefs};
use gtk4::prelude::*;
use std::rc::Rc;

/// Terminal opened in the item itself when it is a directory,
/// otherwise in its parent directory.
fn terminal_dir(f: &gio::File) -> String {
    let is_dir = f.query_file_type(gio::FileQueryInfoFlags::NONE, None::<&gio::Cancellable>)
        == gio::FileType::Directory;
    let target = if is_dir { Some(f.clone()) } else { f.parent() };
    target
        .and_then(|p| p.path())
        .unwrap_or_else(glib::home_dir)
        .to_string_lossy()
        .into_owned()
}

/// Register the rename action: a prompt for the new name; the icon
/// keeps its position.
fn rename_action(group: &gio::SimpleActionGroup, s: &Rc<Surface>, item: &Item, f: &gio::File) {
    let (s, f, old_id, cur) = (s.clone(), f.clone(), item.id(), item.name.clone());
    action(group, "rename", move || {
        let (s, f, old_id) = (s.clone(), f.clone(), old_id.clone());
        crate::dialogs::name_prompt(
            &tr("Rename"),
            &cur,
            Rc::new(move |n| {
                if !n.is_empty() {
                    if let Ok(new) = f.set_display_name(&n, None::<&gio::Cancellable>) {
                        s.rename_position(&old_id, new.uri().as_str());
                    }
                }
                for s in s.all_surfaces() {
                    s.refresh();
                }
            }),
        );
    });
}

/// Register the delete action: trash directly, or delete for good
/// after a confirmation — as chosen by the user.
fn delete_action(group: &gio::SimpleActionGroup, s: &Rc<Surface>, item: &Item, how: DeleteAction) {
    let (s, it) = (s.clone(), item.clone());
    action(group, "delete", move || {
        let (s, it) = (s.clone(), it.clone());
        let name = it.name.clone();
        let run = Rc::new(move || {
            match how {
                DeleteAction::Trash => items::trash(&it),
                DeleteAction::Delete => items::delete_permanently(&it),
            }
            s.forget_position(&it.id());
            for s in s.all_surfaces() {
                s.refresh();
            }
        });
        if how == DeleteAction::Trash {
            return run();
        }
        let body = format!(
            "{} \"{}\"",
            tr("This permanently deletes and cannot be undone:"),
            name
        );
        crate::dialogs::confirm(&tr("Delete Permanently"), &body, &tr("Delete"), run);
    });
}

/// Show the icon menu at `pos`, parented to `parent` — the window
/// the clicked widget actually lives on, which may be another
/// monitor's surface than the one in `s` after a cross-drag.
pub fn file(s: &Rc<Surface>, parent: &impl IsA<gtk4::Widget>, pos: (f64, f64), item: &Item) {
    let prefs = Prefs::load();
    let group = gio::SimpleActionGroup::new();
    // Flat single-section layout on purpose: every GMenu section makes
    // GTK's popover menu measure ~13 px short, which forces scrolling.
    let menu = gio::Menu::new();

    let it = item.clone();
    action(&group, "open", move || items::open(&it));
    menu.append(Some(&tr("Open")), Some("ctx.open"));
    if let Some(f) = &item.file {
        if prefs.shows("open-with") {
            let f2 = f.clone();
            action(&group, "open-with", move || crate::dialogs::open_with(&f2));
            menu.append(Some(&tr("Open With…")), Some("ctx.open-with"));
        }
        if prefs.shows("terminal") {
            let f3 = f.clone();
            action(&group, "terminal", move || {
                spawn(&format!(
                    "sh -c 'cd \"{}\" && for t in gnome-terminal kgx kitty alacritty xterm; do command -v $t >/dev/null && exec $t; done'",
                    terminal_dir(&f3)
                ));
            });
            menu.append(Some(&tr("Open in Terminal")), Some("ctx.terminal"));
        }
    }

    if let (true, Some(f)) = (item.is_file(), &item.file) {
        if prefs.shows("rename") {
            rename_action(&group, s, item, f);
            menu.append(Some(&tr("Rename…")), Some("ctx.rename"));
        }
        delete_action(&group, s, item, prefs.delete);
        let label = match prefs.delete {
            DeleteAction::Trash => tr("Move to Trash"),
            DeleteAction::Delete => tr("Delete Permanently"),
        };
        menu.append(Some(&label), Some("ctx.delete"));
    }
    show(parent, &menu, &group, pos);
}
