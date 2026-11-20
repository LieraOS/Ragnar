//! Ragnar Edit
//!
//! Copyright (C) 2026 Jörg Schröder
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
//! General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Native Wayland text editor for the Ragnar desktop — Rust/GTK4
//! successor to the Qt6 LieraEdit. Keeps the original layout: menu
//! bar (File/Edit/View/Settings/Help), document tabs with dirty
//! indicators and a status bar (Ready | Line: x | Column: y).

mod actions;
mod editor;
mod fileio;
mod highlight;
mod i18n;
mod keywords;
mod print;
mod search;
mod settings;
mod settingsdlg;
mod tabs;

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "org.lieraos.ragnar.Edit";

/// Everything the open/activate handlers need to reach into.
struct Ctx {
    window: adw::ApplicationWindow,
    tabs: Rc<tabs::Tabs>,
    set_status: Rc<dyn Fn(String)>,
}

fn main() {
    i18n::init();
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let ctx: Rc<RefCell<Option<Ctx>>> = Rc::new(RefCell::new(None));
    {
        let ctx = ctx.clone();
        app.connect_activate(move |a| {
            ensure(a, &ctx).window.present();
        });
    }
    {
        let ctx = ctx.clone();
        // "Open with" / file args: each file becomes a tab.
        app.connect_open(move |a, files, _| {
            let c = ensure(a, &ctx);
            for f in files {
                if let Some(p) = f.path() {
                    actions::load_into_tab(&c.tabs, p, c.set_status.clone());
                }
            }
            c.window.present();
        });
    }
    app.run();
}

/// Lazily builds the single application window on first use.
fn ensure<'a>(
    app: &adw::Application,
    ctx: &'a Rc<RefCell<Option<Ctx>>>,
) -> std::cell::Ref<'a, Ctx> {
    if ctx.borrow().is_none() {
        *ctx.borrow_mut() = Some(build_ui(app));
    }
    std::cell::Ref::map(ctx.borrow(), |c| c.as_ref().unwrap())
}

fn build_ui(app: &adw::Application) -> Ctx {
    let settings = Rc::new(RefCell::new(settings::load()));
    adw::StyleManager::default().set_color_scheme(if settings.borrow().dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::PreferLight
    });

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Ragnar Edit")
        .default_width(1000)
        .default_height(700)
        .build();

    let tabs = Rc::new(tabs::new());

    // ---- menu bar: File / Edit / View / Settings / Help ----
    let menu = gio::Menu::new();
    let file = gio::Menu::new();
    file.append(Some(gettext("New").as_str()), Some("win.new"));
    file.append(Some(gettext("Open…").as_str()), Some("win.open"));
    file.append(Some(gettext("Save").as_str()), Some("win.save"));
    file.append(Some(gettext("Save As…").as_str()), Some("win.save-as"));
    file.append(Some(gettext("Print…").as_str()), Some("win.print"));
    file.append(Some(gettext("Exit").as_str()), Some("win.quit"));
    let edit = gio::Menu::new();
    edit.append(Some(gettext("Undo").as_str()), Some("win.undo"));
    edit.append(Some(gettext("Redo").as_str()), Some("win.redo"));
    edit.append(Some(gettext("Cut").as_str()), Some("win.cut"));
    edit.append(Some(gettext("Copy").as_str()), Some("win.copy"));
    edit.append(Some(gettext("Paste").as_str()), Some("win.paste"));
    edit.append(
        Some(gettext("Find and Replace…").as_str()),
        Some("win.find"),
    );
    let view = gio::Menu::new();
    view.append(Some(gettext("Zoom In").as_str()), Some("win.zoom-in"));
    view.append(Some(gettext("Zoom Out").as_str()), Some("win.zoom-out"));
    view.append(Some(gettext("Reset Zoom").as_str()), Some("win.zoom-reset"));
    let tools = gio::Menu::new();
    tools.append(Some(gettext("Settings…").as_str()), Some("win.settings"));
    let help = gio::Menu::new();
    help.append(
        Some(gettext("About Ragnar Edit").as_str()),
        Some("win.about"),
    );
    menu.append_submenu(Some(gettext("File").as_str()), &file);
    menu.append_submenu(Some(gettext("Edit").as_str()), &edit);
    menu.append_submenu(Some(gettext("View").as_str()), &view);
    menu.append_submenu(Some(gettext("Settings").as_str()), &tools);
    menu.append_submenu(Some(gettext("Help").as_str()), &help);
    let menubar = gtk4::PopoverMenuBar::from_model(Some(&menu));

    // Real title bar — without it the window cannot be grabbed or
    // moved on Wayland, and the original shows title + window buttons.
    let wtitle = adw::WindowTitle::new("Ragnar Edit", "");
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&wtitle));

    // ---- status bar ----
    let status = gtk4::Label::new(Some(gettext("Ready").as_str()));
    status.set_halign(gtk4::Align::Start);
    status.set_hexpand(true);
    let pos = gtk4::Label::new(Some("Line: 1 | Column: 1"));
    pos.add_css_class("dim-label");
    let sbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    sbar.add_css_class("statusbar");
    sbar.append(&status);
    sbar.append(&pos);

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&menubar);
    root.append(&tabs.bar);
    root.append(&tabs.view);
    root.append(&sbar);
    window.set_content(Some(&root));
    install_font_css(&settings.borrow().font, 1.0);
    window.present();

    // Close guard: never drop unsaved documents silently.
    {
        let tw = tabs.clone();
        window.connect_close_request(move |w| {
            let dirty: Vec<_> = tabs::all(&tw)
                .into_iter()
                .filter(|(e, _)| *e.dirty.borrow())
                .collect();
            if dirty.is_empty() {
                return glib::Propagation::Proceed;
            }
            let d = adw::AlertDialog::new(
                Some(&gettext("Unsaved changes")),
                Some(&gettext(
                    "Some documents have unsaved changes. Save them before closing?",
                )),
            );
            d.add_response("cancel", &gettext("Cancel"));
            d.add_response("discard", &gettext("Discard changes"));
            d.add_response("save", &gettext("Save and close"));
            d.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
            d.set_default_response(Some("save"));
            d.set_close_response("cancel");
            let (tw2, w2) = (tw.clone(), w.clone());
            d.connect_response(None, move |_, r| match r {
                "discard" => w2.destroy(),
                "save" => actions::save_all_then_close(&tw2, &w2),
                _ => {}
            });
            d.present(Some(w));
            glib::Propagation::Stop
        });
    }

    let set_status: Rc<dyn Fn(String)> = Rc::new({
        let status = status.clone();
        move |m| status.set_text(&m)
    });
    let update_pos: Rc<dyn Fn()> = Rc::new({
        let (pos, tabs) = (pos.clone(), tabs.clone());
        move || {
            if let Some((ed, _)) = tabs::current(&tabs) {
                let (l, c) = editor::cursor_line_col(&ed);
                pos.set_text(&format!("Line: {} | Column: {}", l, c));
            }
        }
    });

    // Drag & drop: files dropped anywhere on the window open as tabs —
    // same as the Qt original's acceptDrops behaviour.
    {
        let (t4, st4) = (tabs.clone(), set_status.clone());
        let drop = gtk4::DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );
        drop.connect_drop(move |_, v, _, _| {
            if let Ok(fl) = v.get::<gtk4::gdk::FileList>() {
                for f in fl.files() {
                    if let Some(p) = f.path() {
                        actions::load_into_tab(&t4, p, st4.clone());
                    }
                }
                true
            } else {
                false
            }
        });
        root.add_controller(drop);
    }

    // first tab + wiring for future tabs' dirty/cursor callbacks
    let new_tab: Rc<dyn Fn() -> adw::TabPage> = Rc::new({
        let (tabs, settings, set_status, update_pos) = (
            tabs.clone(),
            settings.clone(),
            set_status.clone(),
            update_pos.clone(),
        );
        move || {
            let s = settings.borrow();
            let (ed, page) = tabs::add(&tabs, &s.font, s.word_wrap, s.line_numbers);
            drop(s);
            let upd = update_pos.clone();
            *ed.on_cursor.borrow_mut() = Box::new(move || upd());
            {
                let (tt, ss) = (tabs.clone(), set_status.clone());
                *ed.on_drop_file.borrow_mut() =
                    Box::new(move |p| actions::load_into_tab(&tt, p, ss.clone()));
            }
            let (pg, st) = (page.clone(), set_status.clone());
            *ed.on_dirty.borrow_mut() = Box::new(move || {
                tabs::set_dirty_indicator(&pg, true);
                st(gettext("Modified"));
            });
            // debounced re-highlight on change
            let edc = ed.clone();
            let pending = Rc::new(RefCell::new(false));
            let p2 = pending.clone();
            ed.view.buffer().connect_changed(move |b| {
                if *p2.borrow() {
                    return;
                }
                *p2.borrow_mut() = true;
                let (ed3, p3, buf) = (edc.clone(), p2.clone(), b.clone());
                glib::timeout_add_local_once(Duration::from_millis(900), move || {
                    *p3.borrow_mut() = false;
                    let lang = ed3
                        .path
                        .borrow()
                        .as_ref()
                        .and_then(|p| p.extension().and_then(|e| e.to_str()))
                        .map(highlight::from_ext)
                        .unwrap_or(highlight::Lang::Plain);
                    highlight::apply(&buf, lang);
                    let _ = b;
                });
            });
            page
        }
    });
    // retitle + cursor label on tab switches
    {
        let (t3, wt, up) = (tabs.clone(), wtitle.clone(), update_pos.clone());
        tabs.view.connect_selected_page_notify(move |_| {
            if let Some((_e, p)) = tabs::current(&t3) {
                wt.set_subtitle(&p.title());
            } else {
                wt.set_subtitle("");
            }
            up();
        });
    }
    new_tab();

    actions::wire_actions(&window, app, &tabs, &settings, &new_tab, &set_status);

    // autosave
    let (t2, s2, st2) = (tabs.clone(), settings.clone(), set_status.clone());
    glib::timeout_add_local(Duration::from_secs(2), move || {
        let secs = s2.borrow().autosave_secs;
        if secs == 0 {
            return glib::ControlFlow::Continue;
        }
        for (ed, page) in tabs::all(&t2) {
            if *ed.dirty.borrow() {
                if let Some(p) = ed.path.borrow().clone() {
                    if actions::save_editor(&ed, &p).is_ok() {
                        *ed.dirty.borrow_mut() = false;
                        tabs::set_dirty_indicator(&page, false);
                        st2(gettext("Auto-saved"));
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    });
    Ctx {
        window,
        tabs,
        set_status,
    }
}

/// Writes CSS that applies the configured editor font + zoom factor.
pub fn install_font_css(font: &str, zoom: f64) {
    // split "Monospace 11" → family "Monospace", size 11pt
    let (fam, size) = font
        .rsplit_once(' ')
        .map(|(f, sz)| (f.to_string(), sz.parse::<f64>().unwrap_or(11.0)))
        .unwrap_or((font.to_string(), 11.0));
    let css = format!(
        ".ragnar-edit-view text {{ font-family: {}; font-size: {}pt; }}\n.statusbar {{ padding: 4px 8px; border-top: 1px solid alpha(@borders, .35); }}",
        fam, size * zoom
    );
    let prov = gtk4::CssProvider::new();
    prov.load_from_data(&css);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &prov,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
