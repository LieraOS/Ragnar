// Start-menu settings page — same card look the bar pages use.
// Behaviour card: switch mode + category column toggle. Manager
// card: category list on the left (standard + custom, add/delete)
// and the app checklist on the right — ticking assigns the app to
// the selected category exclusively, unticking drops the override.
// The selected category lives on `ui.menu_sel` so page rebuilds keep
// it; a single checkbox only writes menu.ini, no UI rebuild.
use super::Ui;
use crate::menucfg::{CustomCat, MenuCfg};
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Standard categories in menu order — identical to the labels the
/// start menu uses so assignments line up one-to-one.
const STD_CATS: &[&str] = &[
    "Audio & Video",
    "Entwicklung",
    "Bildung",
    "Spiele",
    "Grafik",
    "Internet",
    "Büro",
    "Einstellungen",
    "System",
    "Zubehör",
    "Sonstige",
];

/// Mutate the menu config and persist — the single write path.
fn edit(mutate: impl FnOnce(&mut MenuCfg)) {
    let mut cfg = crate::menucfg::load();
    mutate(&mut cfg);
    crate::menucfg::save(&cfg);
}

/// A section wrapped in the shared card styling.
fn card(child: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let c = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    c.add_css_class("settings-card");
    c.set_margin_top(6);
    c.set_margin_bottom(6);
    c.append(child);
    c
}

fn head(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.set_halign(gtk4::Align::Start);
    l.add_css_class("heading");
    l
}

/// The whole start-menu page shown in the detail pane.
pub fn build(ui: &Rc<Ui>) -> gtk4::Widget {
    let cfg = crate::menucfg::load();
    let col = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    col.set_margin_top(12);
    col.set_margin_bottom(12);
    col.set_margin_start(14);
    col.set_margin_end(14);

    // --- Behaviour card ---
    let behav = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    behav.append(&head(&crate::i18n::tr("Verhalten")));
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let hover = gtk4::ToggleButton::with_label(&crate::i18n::tr("Bei Mauskontakt wechseln"));
    let click = gtk4::ToggleButton::with_label(&crate::i18n::tr("Beim Klicken wechseln"));
    click.set_group(Some(&hover));
    hover.set_active(cfg.switch_hover);
    click.set_active(!cfg.switch_hover);
    hover.connect_toggled(|b| {
        if b.is_active() {
            edit(|c| c.switch_hover = true);
        }
    });
    click.connect_toggled(|b| {
        if b.is_active() {
            edit(|c| c.switch_hover = false);
        }
    });
    row.append(&hover);
    row.append(&click);
    behav.append(&row);
    let show = gtk4::CheckButton::with_label(&crate::i18n::tr("Kategorien anzeigen"));
    show.set_active(cfg.show_categories);
    show.connect_toggled(|b| edit(|c| c.show_categories = b.is_active()));
    behav.append(&show);
    col.append(&card(&behav));

    // --- Category & app manager card ---
    let mgr = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    mgr.append(&head(&crate::i18n::tr("Kategorien & Apps")));
    mgr.append(&assign_section(ui));
    let wrap = card(&mgr);
    wrap.set_vexpand(true);
    col.append(&wrap);
    col.upcast()
}

/// Category list (left) + lazy app checklist (right). The selected
/// name comes from `ui.menu_sel` so it survives page rebuilds.
fn assign_section(ui: &Rc<Ui>) -> gtk4::Widget {
    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let cat_col = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    cat_col.set_size_request(160, -1);
    let cat_scroll = gtk4::ScrolledWindow::new();
    cat_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
    cat_scroll.set_vexpand(true);
    cat_scroll.set_child(Some(&cat_col));
    let app_holder = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    app_holder.set_hexpand(true);
    app_holder.set_vexpand(true);

    let cfg = crate::menucfg::load();
    let current = ui.menu_sel.borrow().clone();
    let cat_btns = std::rc::Rc::new(std::cell::RefCell::new(Vec::<gtk4::Button>::new()));
    // The middle list's selected app id — feeds the right-hand
    // Eigenschaften/Löschen buttons.
    let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    // "Alle Anwendungen" — the mozo-style root: shows every app so it
    // can be dragged onto a category or edited.
    {
        let btn = gtk4::Button::with_label(&crate::i18n::tr("Alle Anwendungen"));
        btn.set_has_frame(false);
        if let Some(l) = btn.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
            l.set_xalign(0.0);
        }
        if current == "Alle Anwendungen" {
            btn.add_css_class("selected");
        }
        cat_btns.borrow_mut().push(btn.clone());
        {
            let (u, ah, all_btns, sel) = (
                ui.clone(),
                app_holder.clone(),
                cat_btns.clone(),
                selected.clone(),
            );
            btn.connect_clicked(move |b| {
                *u.menu_sel.borrow_mut() = "Alle Anwendungen".to_string();
                for o in all_btns.borrow().iter() {
                    o.remove_css_class("selected");
                }
                b.add_css_class("selected");
                rebuild_apps(&u, "Alle Anwendungen", &ah, sel.clone());
            });
        }
        cat_col.append(&btn);
    }
    // Category names in display order: cfg.cat_order wins, built-in
    // order fills gaps, custom cats without an order entry go last.
    let mut names: Vec<String> = Vec::new();
    for n in &cfg.cat_order {
        names.push(n.clone());
    }
    for n in STD_CATS.iter().map(|s| s.to_string()) {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    for c in &cfg.custom {
        if !names.contains(&c.name) {
            names.push(c.name.clone());
        }
    }
    for name in names {
        let is_custom = cfg.custom.iter().any(|c| c.name == name);
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);

        // Drag handle — grabbing it starts a reorder drag; the row
        // itself stays a normal button for selection.
        let handle = gtk4::Image::from_icon_name("list-drag-handle-symbolic");
        handle.add_css_class("dim-label");
        let drag = gtk4::DragSource::new();
        drag.set_actions(gtk4::gdk::DragAction::MOVE);
        let nm_drag = name.clone();
        drag.connect_prepare(move |_, _, _| {
            Some(gtk4::gdk::ContentProvider::for_value(
                &format!("cat:{}", nm_drag).to_value(),
            ))
        });
        handle.add_controller(drag);
        row.append(&handle);

        let btn = gtk4::Button::with_label(&name);
        btn.set_has_frame(false);
        btn.set_hexpand(true);
        if let Some(l) = btn.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
            l.set_xalign(0.0);
        }
        if name == current {
            btn.add_css_class("selected");
        }
        cat_btns.borrow_mut().push(btn.clone());
        {
            let (u, ah, all_btns, sel) = (
                ui.clone(),
                app_holder.clone(),
                cat_btns.clone(),
                selected.clone(),
            );
            btn.connect_clicked(move |b| {
                *u.menu_sel.borrow_mut() = b.label().unwrap().to_string();
                for o in all_btns.borrow().iter() {
                    o.remove_css_class("selected");
                }
                b.add_css_class("selected");
                rebuild_apps(&u, &u.menu_sel.borrow(), &ah, sel.clone());
            });
        }
        // Drop target on the row: "cat:<name>" reorders the list,
        // a bare desktop id assigns that app to this category.
        let dt =
            gtk4::DropTarget::new(gtk4::glib::types::Type::STRING, gtk4::gdk::DragAction::MOVE);
        let (nm2, u2) = (name.clone(), ui.clone());
        dt.connect_drop(move |_, val, _, _| {
            let Ok(payload) = val.get::<String>() else {
                return false;
            };
            if let Some(src) = payload.strip_prefix("cat:") {
                let (src, dst) = (src.to_string(), nm2.clone());
                if src == dst {
                    return false;
                }
                edit(move |c| {
                    // Seed the order with the current full list when
                    // the user reorders for the first time.
                    if c.cat_order.is_empty() {
                        c.cat_order = STD_CATS
                            .iter()
                            .map(|s| s.to_string())
                            .chain(c.custom.iter().map(|x| x.name.clone()))
                            .collect();
                    }
                    // Move src to the position dst currently holds:
                    // drop src, insert it right before dst.
                    c.cat_order.retain(|n| *n != src);
                    let i = c
                        .cat_order
                        .iter()
                        .position(|n| *n == dst)
                        .unwrap_or(c.cat_order.len());
                    c.cat_order.insert(i, src);
                });
                super::rebuild_all(&u2);
                return true;
            }
            let nm = nm2.clone();
            edit(move |c| {
                c.map.insert(payload, nm);
            });
            true
        });
        row.add_controller(dt);
        row.append(&btn);
        if is_custom {
            let del = gtk4::Button::from_icon_name("user-trash-symbolic");
            del.set_has_frame(false);
            let (u, nm) = (ui.clone(), name.clone());
            del.connect_clicked(move |_| {
                edit(|c| {
                    c.custom.retain(|x| x.name != nm);
                    c.map.retain(|_, v| *v != nm);
                    c.cat_order.retain(|n| *n != nm);
                });
                super::rebuild_all(&u);
            });
            row.append(&del);
        }
        cat_col.append(&row);
    }

    // "+ Neue Kategorie" — inline entry + button at the list's foot.
    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("Neue Kategorie …"));
    entry.set_hexpand(true);
    let addb = gtk4::Button::with_label(&crate::i18n::tr("+"));
    let commit_add = {
        let (u, ent) = (ui.clone(), entry.clone());
        move || {
            let name = ent.text().trim().to_string();
            let exists = {
                let c = crate::menucfg::load();
                c.custom.iter().any(|x| x.name == name) || STD_CATS.iter().any(|s| *s == name)
            };
            if !name.is_empty() && !exists {
                ent.set_text("");
                edit(|c| c.custom.push(CustomCat { name }));
                super::rebuild_all(&u);
            }
        }
    };
    let add = std::rc::Rc::new(commit_add);
    let add2 = add.clone();
    addb.connect_clicked(move |_| add2());
    entry.connect_activate(move |_| add());
    let addrow = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    addrow.append(&entry);
    addrow.append(&addb);
    cat_col.append(&addrow);

    body.append(&cat_scroll);
    body.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    body.append(&app_holder);
    // mozo-style button column on the right: actions on the selected
    // middle-list entry plus launcher creation.
    body.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    let btns = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    btns.set_valign(gtk4::Align::Start);
    let new_entry = gtk4::Button::with_label(&crate::i18n::tr("Neuer Eintrag"));
    let props = gtk4::Button::with_label(&crate::i18n::tr("Eigenschaften"));
    let del = gtk4::Button::with_label(&crate::i18n::tr("Löschen"));
    {
        let u = ui.clone();
        new_entry.connect_clicked(move |_| {
            let fields = super::launcher_dlg::Fields {
                id: None,
                name: String::new(),
                exec: String::new(),
                comment: String::new(),
                icon: String::new(),
                terminal: false,
                gpu: false,
            };
            super::launcher_dlg::open(&u.win, fields, {
                let u = u.clone();
                std::rc::Rc::new(move |f| {
                    super::launcher_dlg::save(&f);
                    super::rebuild_all(&u);
                })
            });
        });
    }
    {
        let (u, sel) = (ui.clone(), selected.clone());
        props.connect_clicked(move |_| {
            let Some(id) = sel.borrow().clone() else {
                return;
            };
            super::launcher_dlg::open(&u.win, super::launcher_dlg::for_app(&id), {
                let u = u.clone();
                std::rc::Rc::new(move |f| {
                    super::launcher_dlg::save(&f);
                    super::rebuild_all(&u);
                })
            });
        });
    }
    {
        let (u, sel) = (ui.clone(), selected.clone());
        del.connect_clicked(move |_| {
            let Some(id) = sel.borrow().clone() else {
                return;
            };
            // A user-level file gets deleted; a system app is hidden
            // instead — removal is destructive, hiding is reversible.
            let user_file = gio::DesktopAppInfo::new(id.as_str())
                .and_then(|ai| ai.filename())
                .map(|p| p.to_string_lossy().contains(".local/share/applications"))
                .unwrap_or(false);
            if user_file {
                let _ = std::fs::remove_file(override_path(&id));
            }
            edit(move |c| {
                if user_file {
                    c.map.remove(&id);
                    c.renames.remove(&id);
                    c.hidden.remove(&id);
                } else {
                    c.hidden.insert(id);
                }
            });
            super::rebuild_all(&u);
        });
    }
    btns.append(&new_entry);
    btns.append(&props);
    btns.append(&del);
    body.append(&btns);
    rebuild_apps(ui, &ui.menu_sel.borrow(), &app_holder, selected);
    body.upcast()
}

/// Fill the middle list for `cat`: "Alle Anwendungen" shows every
/// app, a real category shows its members. The leading checkbox is
/// the "Anzeigen" state (maps to cfg.hidden); selection feeds the
/// Eigenschaften/Löschen buttons on the right.
fn rebuild_apps(ui: &Rc<Ui>, cat: &str, holder: &gtk4::Box, selected: Rc<RefCell<Option<String>>>) {
    while let Some(c) = holder.first_child() {
        holder.remove(&c);
    }
    let cfg = crate::menucfg::load();
    let mut seen: HashSet<String> = HashSet::new();
    let mut apps: Vec<gio::AppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter(|a| {
            a.should_show() && seen.insert(a.id().map(|s| s.to_string()).unwrap_or_default())
        })
        .collect();
    apps.sort_by_key(|a| a.name().to_lowercase());
    let all_view = cat == "Alle Anwendungen";

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list = gtk4::ListBox::new();
    {
        let sel = selected.clone();
        list.connect_row_selected(move |_, row| {
            *sel.borrow_mut() = row.and_then(|r| {
                unsafe { r.data::<String>("id") }.map(|d| unsafe { d.as_ref() }.clone())
            });
        });
    }
    for app in apps {
        let id = app.id().map(|s| s.to_string()).unwrap_or_default();
        let eff = effective_cat(&cfg, &app);
        if !all_view && eff != cat {
            continue;
        }
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        // "Anzeigen" — unticking hides the app from the menu.
        let cb = gtk4::CheckButton::new();
        cb.set_active(!cfg.hidden.contains(&id));
        let id_h = id.clone();
        cb.connect_toggled(move |b| {
            let i = id_h.clone();
            edit(move |c| {
                if b.is_active() {
                    c.hidden.remove(&i);
                } else {
                    c.hidden.insert(i);
                }
            });
        });
        // The app's real icon — easier to spot than bare names.
        let icon = app
            .icon()
            .map(|i| gtk4::Image::from_gicon(&i))
            .unwrap_or_else(|| gtk4::Image::from_icon_name("application-x-executable-symbolic"));
        icon.set_pixel_size(16);
        let shown = cfg
            .renames
            .get(&id)
            .cloned()
            .unwrap_or_else(|| app.name().to_string());
        let nm = gtk4::Label::new(Some(&shown));
        nm.set_hexpand(true);
        nm.set_xalign(0.0);
        let tag = gtk4::Label::new(Some(&eff));
        tag.add_css_class("dim-label");
        row.append(&cb);
        row.append(&icon);
        row.append(&nm);
        row.append(&tag);

        // Right-click the row → the launcher properties dialog.
        let click = gtk4::GestureClick::new();
        click.set_button(3);
        let (u, id_r) = (ui.clone(), id.clone());
        click.connect_pressed(move |g, _, _, _| {
            super::launcher_dlg::open(&u.win, super::launcher_dlg::for_app(&id_r), {
                let u = u.clone();
                std::rc::Rc::new(move |f| {
                    super::launcher_dlg::save(&f);
                    super::rebuild_all(&u);
                })
            });
            g.set_state(gtk4::EventSequenceState::Claimed);
        });
        row.add_controller(click);

        // Drag source: the app id is the payload dropped on category
        // buttons in the left column.
        let drag = gtk4::DragSource::new();
        drag.set_actions(gtk4::gdk::DragAction::MOVE);
        let id5 = id.clone();
        drag.connect_prepare(move |_, _, _| {
            Some(gtk4::gdk::ContentProvider::for_value(
                &id5.clone().to_value(),
            ))
        });
        nm.add_controller(drag);

        let cb_row = gtk4::ListBoxRow::new();
        unsafe { cb_row.set_data("id", id) };
        cb_row.set_child(Some(&row));
        list.append(&cb_row);
    }
    scroll.set_child(Some(&list));
    holder.append(&scroll);
}

/// Path of the per-user override file for a desktop id.
fn override_path(id: &str) -> std::path::PathBuf {
    gtk4::glib::user_data_dir().join("applications").join(id)
}

/// The category an app lands in right now: an explicit user
/// assignment wins, otherwise its .desktop category (or Sonstige).
fn effective_cat(cfg: &MenuCfg, app: &gio::AppInfo) -> String {
    let id = app.id().map(|s| s.to_string()).unwrap_or_default();
    if let Some(name) = cfg.map.get(&id) {
        return name.clone();
    }
    let cats = app
        .clone()
        .downcast::<gio::DesktopAppInfo>()
        .ok()
        .and_then(|d| d.categories())
        .map(|c| c.to_string())
        .unwrap_or_default();
    for (raw, label) in [
        ("AudioVideo", "Audio & Video"),
        ("Development", "Entwicklung"),
        ("Education", "Bildung"),
        ("Game", "Spiele"),
        ("Graphics", "Grafik"),
        ("Network", "Internet"),
        ("Office", "Büro"),
        ("Settings", "Einstellungen"),
        ("System", "System"),
        ("Utility", "Zubehör"),
    ] {
        if cats.split(';').any(|x| x == raw) {
            return label.to_string();
        }
    }
    "Sonstige".into()
}
