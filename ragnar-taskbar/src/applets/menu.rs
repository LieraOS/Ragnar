// Start-menu applet: the Ragnar logo button opens the menu inside
// the shared menu overlay. Layout: search field on top, a category
// column on the left and a GtkStack of app lists on the right —
// hovering a category switches the stack (the switch mode becomes
// configurable in the taskbar settings later). A session strip at
// the bottom holds two icon buttons: Ragnar Settings and the
// session power dialog (ragnar-power-dialog offers the individual
// actions). All data is real: apps come from gio::AppInfo and
// categories from the .desktop Categories fields.
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::Cell;

/// The Ragnar logo, embedded so the taskbar never depends on a file
/// path at runtime (scaled down to 128px — plenty for HiDPI too).
const LOGO: &[u8] = include_bytes!("../../data/ragnar.png");

/// freedesktop main category → (sort key, German label). Apps whose
/// .desktop declares none of these land in "Sonstige".
const CATS: &[(&str, &str)] = &[
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
];

/// The start button; `edge` is the bar's docked edge so the menu
/// overlay can anchor itself at the button on any side.
pub fn build(icon_size: i32, edge: &str) -> gtk4::Widget {
    let button = gtk4::Button::new();
    let tex = gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from_static(LOGO))
        .expect("embedded ragnar.png must decode");
    let img = gtk4::Image::from_paintable(Some(&tex));
    img.set_pixel_size(icon_size);
    button.set_child(Some(&img));
    button.add_css_class("menu-button");
    button.set_tooltip_text(Some(&crate::i18n::tr("Startmenü")));

    let edge = edge.to_string();
    button.connect_clicked(move |b| {
        let win = crate::menu::shell_at(b, &edge);
        crate::menu::show(&win, &content(icon_size));
    });

    button.upcast()
}

/// The whole menu: search on top, categories + app stack in the
/// middle, session strip at the bottom.
fn content(icon_size: i32) -> gtk4::Widget {
    let cfg = crate::menucfg::load();
    let layout = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    layout.set_size_request(460, 480);
    layout.set_margin_top(4);
    layout.set_margin_bottom(4);
    layout.set_margin_start(4);
    layout.set_margin_end(4);

    let search = gtk4::SearchEntry::new();
    layout.append(&search);

    // Every app lands in exactly one bucket: an explicit user
    // assignment (cfg.map, matches the display label) wins, then the
    // first known main category, or "other" when the .desktop
    // declares none. Custom categories get buckets of their own.
    let mut cats: Vec<(String, String, Vec<gio::AppInfo>)> = CATS
        .iter()
        .map(|(id, name)| (id.to_string(), name.to_string(), Vec::new()))
        .collect();
    cats.push(("other".to_string(), "Sonstige".to_string(), Vec::new()));
    for c in &cfg.custom {
        cats.push((format!("custom:{}", c.name), c.name.clone(), Vec::new()));
    }
    let mut all: Vec<gio::AppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter(|a| {
            a.should_show()
                && !cfg
                    .hidden
                    .contains(&a.id().map(|s| s.to_string()).unwrap_or_default())
        })
        .collect();
    all.sort_by_key(|a| a.name().to_lowercase());
    for app in &all {
        let app_id = app.id().map(|s| s.to_string()).unwrap_or_default();
        // A user assignment names the category by its display label —
        // that covers standard and custom categories alike.
        if let Some(cat_name) = cfg.map.get(&app_id) {
            if let Some(bucket) = cats.iter_mut().find(|(_, n, _)| n == cat_name) {
                bucket.2.push(app.clone());
                continue;
            }
        }
        let key = app
            .clone()
            .downcast::<gio::DesktopAppInfo>()
            .ok()
            .and_then(|d| d.categories())
            .map(|c| c.to_string())
            .and_then(|c| {
                CATS.iter()
                    .find(|(id, _)| c.split(';').any(|x| x == *id))
                    .map(|(id, _)| id.to_string())
            })
            .unwrap_or_else(|| "other".to_string());
        if let Some(bucket) = cats.iter_mut().find(|(id, _, _)| *id == key) {
            bucket.2.push(app.clone());
        }
    }
    // User-defined display order — unknown names keep their built-in
    // position, new custom cats without an order entry go last.
    if !cfg.cat_order.is_empty() {
        let order = &cfg.cat_order;
        cats.sort_by_key(|(_, name, _)| order.iter().position(|o| o == name).unwrap_or(usize::MAX));
    }

    // One list per used category inside a stack — switching pages is
    // instant, which is what makes hover-switching feel right.
    let stack = gtk4::Stack::new();
    let cat_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

    // crate::i18n::tr("Alle Anwendungen").as_str() first — a flat list of everything.
    add_cat_page(
        &stack,
        "all",
        crate::i18n::tr("Alle Anwendungen").as_str(),
        &all_apps(&cats),
        icon_size,
        &cfg.renames,
    );
    cat_col.append(&cat_button(
        &stack,
        "all",
        crate::i18n::tr("Alle Anwendungen").as_str(),
        cfg.switch_hover,
    ));
    for (id, name, apps) in &cats {
        if apps.is_empty() {
            continue;
        }
        add_cat_page(
            &stack,
            id,
            &crate::i18n::tr(name),
            apps,
            icon_size,
            &cfg.renames,
        );
        cat_col.append(&cat_button(
            &stack,
            id,
            &crate::i18n::tr(name),
            cfg.switch_hover,
        ));
    }
    stack.set_visible_child_name("all");

    // Global search page — one flat list over every app, rows
    // hidden by name while typing. Hover-switching pauses via
    // SEARCHING so the results stay put.
    let results = gtk4::ListBox::new();
    results.set_selection_mode(gtk4::SelectionMode::None);
    for app in &all {
        results.append(&app_row(app, icon_size, &cfg.renames));
    }
    let search_scroll = gtk4::ScrolledWindow::new();
    search_scroll.set_hexpand(true);
    search_scroll.set_child(Some(&results));
    stack.add_named(&search_scroll, Some("search"));
    results.connect_row_activated(move |_, row| {
        if let Some(info) = unsafe { row.data::<gio::AppInfo>("app") } {
            let info = unsafe { info.as_ref() };
            spawn_app(&launch_cmd(info));
            crate::menu::close_current();
        }
    });
    {
        let (st, res) = (stack.clone(), results.clone());
        search.connect_search_changed(move |e| {
            let q = e.text().to_lowercase();
            SEARCHING.with(|s| s.set(!q.is_empty()));
            if q.is_empty() {
                st.set_visible_child_name("all");
            } else {
                let mut i = 0;
                while let Some(row) = res.row_at_index(i) {
                    let vis = unsafe { row.data::<gio::AppInfo>("app") }
                        .map(|a| unsafe { a.as_ref() }.name().to_lowercase().contains(&q))
                        .unwrap_or(false);
                    row.set_visible(vis);
                    i += 1;
                }
                st.set_visible_child_name("search");
            }
        });
    }
    // Reset on every close path (Escape, outside click, app launch):
    // clearing the entry re-fires search_changed, which sets
    // SEARCHING back to false and returns the stack to "all" — a
    // stale query would otherwise keep hover-switching suppressed.
    search.connect_unmap(|e| e.set_text(""));

    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    body.set_vexpand(true);
    if cfg.show_categories {
        let cat_scroll = gtk4::ScrolledWindow::new();
        cat_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        cat_scroll.set_propagate_natural_width(true);
        cat_scroll.set_size_request(110, -1);
        cat_scroll.set_child(Some(&cat_col));
        body.append(&cat_scroll);
        body.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    }
    body.append(&stack);
    layout.append(&body);

    layout.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    layout.append(&session_strip());
    layout.upcast()
}

/// Flat vector of every app across all categories.
fn all_apps(cats: &[(String, String, Vec<gio::AppInfo>)]) -> Vec<gio::AppInfo> {
    let mut v: Vec<gio::AppInfo> = cats.iter().flat_map(|(_, _, a)| a.clone()).collect();
    v.sort_by_key(|a| a.name().to_lowercase());
    v
}

/// One stack page: scrollable list of app rows for a category.
fn add_cat_page(
    stack: &gtk4::Stack,
    key: &str,
    name: &str,
    apps: &[gio::AppInfo],
    icon_size: i32,
    renames: &std::collections::HashMap<String, String>,
) {
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    for app in apps {
        list.append(&app_row(app, icon_size, renames));
    }
    scroll.set_child(Some(&list));
    stack.add_named(&scroll, Some(key));
    let _ = name;

    // Launch the activated app and close the overlay.
    list.connect_row_activated(move |_, row| {
        if let Some(info) = unsafe { row.data::<gio::AppInfo>("app") } {
            let info = unsafe { info.as_ref() };
            spawn_app(&launch_cmd(info));
            crate::menu::close_current();
        }
    });
}

/// A category entry in the left column — hovering switches the stack
/// page immediately (default behaviour; click-mode becomes a user
/// setting later).
fn cat_button(stack: &gtk4::Stack, key: &str, name: &str, hover: bool) -> gtk4::Widget {
    let row = gtk4::Button::with_label(name);
    row.set_has_frame(false);
    row.set_halign(gtk4::Align::Fill);
    if let Some(l) = row.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
        l.set_xalign(0.0);
    }
    let k = key.to_string();
    // Mouseover switch — only when the user picked hover mode.
    if hover {
        let mot = gtk4::EventControllerMotion::new();
        let (st, kk) = (stack.clone(), k.clone());
        mot.connect_enter(move |_, _, _| {
            if SEARCHING.with(|s| !s.get()) {
                st.set_visible_child_name(&kk);
            }
        });
        row.add_controller(mot);
    }
    // Click also switches — needed for keyboard users and the future
    // click-mode setting; costs nothing while hover is default.
    let st = stack.clone();
    row.connect_clicked(move |_| st.set_visible_child_name(&k));
    row.upcast()
}

thread_local! {
    /// True while the search field has text — hover-switching is
    /// paused then so the result page stays visible.
    static SEARCHING: Cell<bool> = const { Cell::new(false) };
}

/// One app row: icon + name, carries the AppInfo for launching.
fn app_row(
    app: &gio::AppInfo,
    icon_size: i32,
    renames: &std::collections::HashMap<String, String>,
) -> gtk4::Widget {
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let icon = app
        .icon()
        .map(|i| gtk4::Image::from_gicon(&i))
        .unwrap_or_else(|| gtk4::Image::from_icon_name("application-x-executable-symbolic"));
    icon.set_pixel_size(icon_size);
    row_box.append(&icon);
    let id = app.id().map(|s| s.to_string()).unwrap_or_default();
    let name = renames
        .get(&id)
        .cloned()
        .unwrap_or_else(|| app.name().to_string());
    row_box.append(&gtk4::Label::new(Some(&name)));
    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&row_box));
    unsafe { row.set_data("app", app.clone()) };
    row.upcast()
}

/// The session strip: two clean icon buttons at the right edge —
/// Ragnar Settings and the session power dialog (ragnar-power-dialog
/// shows the individual actions: lock, logout, suspend, reboot,
/// shut down). The class `menu-session-button` keeps them themeable.
fn session_strip() -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    row.set_margin_top(4);

    // Push both buttons to the far right.
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    row.append(&spacer);

    for (icon, tip, cmd) in [
        (
            "preferences-system-symbolic",
            "Einstellungen",
            "ragnar-settings",
        ),
        (
            "system-shutdown-symbolic",
            "Ausschalten",
            "ragnar-power-dialog",
        ),
    ] {
        let b = gtk4::Button::from_icon_name(icon);
        b.add_css_class("menu-session-button");
        b.set_tooltip_text(Some(&crate::i18n::tr(tip)));
        b.set_has_frame(false);
        let cmd = cmd.to_string();
        b.connect_clicked(move |_| {
            spawn(&cmd);
            crate::menu::close_current();
        });
        row.append(&b);
    }
    row.upcast()
}

/// Spawn an app command as a DIRECT child of the taskbar and reap it
/// via a child watch. `spawn_command_line_async` double-forks through
/// a zombie-reaping helper, which orphans the child — pkexec then
/// sees its parent as dead ("Refusing to render service to dead
/// parents") and refuses to run. DO_NOT_REAP_CHILD keeps the parent
/// alive; the watch prevents the zombie.
fn spawn_app(cmd: &str) {
    use gtk4::glib::SpawnFlags;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    // Inherit our environment explicitly: glib-rs maps `&[]` to
    // envp=[NULL], NOT to NULL inherit — the child would get no
    // WAYLAND_DISPLAY/PATH/HOME at all and every GUI app dies.
    let env: Vec<PathBuf> = std::env::vars_os()
        .map(|(mut k, v)| {
            k.push(OsString::from("="));
            k.push(v);
            PathBuf::from(k)
        })
        .collect();
    let envp: Vec<&Path> = env.iter().map(PathBuf::as_path).collect();
    match gtk4::glib::spawn_async(
        None::<&Path>,
        &[Path::new("/bin/sh"), Path::new("-c"), Path::new(cmd)],
        &envp,
        SpawnFlags::SEARCH_PATH | SpawnFlags::DO_NOT_REAP_CHILD,
        None::<Box<dyn FnOnce()>>,
    ) {
        Ok(pid) => {
            eprintln!("[menu] spawned: {cmd}");
            gtk4::glib::child_watch_add(pid, move |_, _| {});
        }
        Err(e) => eprintln!("[menu] spawn {cmd} failed: {e}"),
    }
}

/// Build a spawnable command from an AppInfo: take its commandline
/// and drop freedesktop field codes — a menu click supplies no
/// files, so %f/%F/%u/%U and friends expand to nothing.
fn launch_cmd(info: &gio::AppInfo) -> String {
    const CODES: [&str; 13] = [
        "%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m",
    ];
    let raw = info
        .commandline()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| info.executable().to_string_lossy().to_string());
    raw.split_whitespace()
        .filter(|t| !CODES.contains(t))
        .collect::<Vec<_>>()
        .join(" ")
        .replace("%%", "%")
}

/// Launch a command detached — fire and forget, same as the menu's
/// app launching.
fn spawn(cmdline: &str) {
    spawn_app(cmdline);
}
