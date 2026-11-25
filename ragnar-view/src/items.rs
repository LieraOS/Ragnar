// Copyright (C) Jörg Schröder
//! Desktop item model: what the desktop shows — Home, Trash,
//! mounted volumes, and the files inside the user's Desktop dir.

use gio::prelude::*;
use std::cmp::Ordering;

/// What a desktop icon stands for.
#[derive(Clone)]
pub enum Kind {
    Home,
    Trash,
    Mount,
    File(gio::File),
}

/// One desktop icon.
#[derive(Clone)]
pub struct Item {
    pub kind: Kind,
    pub name: String,
    pub icon: gio::Icon,
    /// Backing file for open/trash/rename — set for Home, Trash, File.
    pub file: Option<gio::File>,
    pub mtime: i64,
    pub size: u64,
}

impl Item {
    /// Stable id used to remember the icon's position.
    pub fn id(&self) -> String {
        match &self.kind {
            Kind::Home => "home".into(),
            Kind::Trash => "trash".into(),
            Kind::Mount => format!("mount:{}", self.name),
            Kind::File(f) => f.uri().to_string(),
        }
    }

    /// True for regular files/dirs the user can trash or rename.
    pub fn is_file(&self) -> bool {
        matches!(self.kind, Kind::File(_))
    }
}

/// The user's Desktop directory (e.g. ~/Desktop or ~/Schreibtisch).
pub fn desktop_dir() -> gio::File {
    let dir = glib::user_special_dir(glib::UserDirectory::Desktop)
        .unwrap_or_else(|| glib::home_dir().join("Desktop"));
    gio::File::for_path(dir)
}

/// Sort order chosen in the background menu.
#[derive(Clone, Copy, PartialEq)]
pub enum SortBy {
    Name,
    Modified,
    Size,
    Type,
}

impl SortBy {
    /// Parse the GSettings value (unknown → Name).
    pub fn from_key(k: &str) -> Self {
        match k {
            "modified" => Self::Modified,
            "size" => Self::Size,
            "type" => Self::Type,
            _ => Self::Name,
        }
    }
}

fn cmp_items(a: &Item, b: &Item, by: SortBy) -> Ordering {
    // Special items (Home/Trash/Mounts) always lead the grid.
    let spec = |i: &Item| !i.is_file();
    match (spec(a), spec(b)) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        (true, true) => return a.name.cmp(&b.name),
        _ => {}
    }
    match by {
        SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        SortBy::Modified => b.mtime.cmp(&a.mtime),
        SortBy::Size => b.size.cmp(&a.size),
        SortBy::Type => {
            let ext = |i: &Item| {
                i.file
                    .as_ref()
                    .and_then(|f| f.path())
                    .and_then(|p| p.extension().map(|e| e.to_string_lossy().into_owned()))
                    .unwrap_or_default()
            };
            ext(a).cmp(&ext(b)).then(a.name.cmp(&b.name))
        }
    }
}

/// Collect all desktop items: Home, Trash, volumes, ~/Desktop files.
pub fn enumerate(by: SortBy) -> Vec<Item> {
    let mut items = vec![
        Item {
            kind: Kind::Home,
            name: crate::i18n::tr("Home"),
            icon: gio::Icon::for_string("user-home").unwrap(),
            file: Some(gio::File::for_path(glib::home_dir())),
            mtime: 0,
            size: 0,
        },
        Item {
            kind: Kind::Trash,
            name: crate::i18n::tr("Trash"),
            icon: gio::Icon::for_string("user-trash").unwrap(),
            file: Some(gio::File::for_uri("trash:///")),
            mtime: 0,
            size: 0,
        },
    ];
    // Mounted volumes (USB sticks etc.) — what the old view showed.
    for m in gio::VolumeMonitor::get().mounts() {
        items.push(Item {
            name: m.name().to_string(),
            icon: m.icon(),
            kind: Kind::Mount,
            file: Some(m.root()),
            mtime: 0,
            size: 0,
        });
    }
    // ~/Desktop contents.
    let dir = desktop_dir();
    let Ok(en) = dir.enumerate_children(
        "standard::name,standard::display-name,standard::icon,time::modified,standard::size,standard::type",
        gio::FileQueryInfoFlags::NONE,
        None::<&gio::Cancellable>,
    ) else {
        items.sort_by(|a, b| cmp_items(a, b, by));
        return items;
    };
    while let Ok(Some(info)) = en.next_file(None::<&gio::Cancellable>) {
        if info.name().to_string_lossy().starts_with('.') {
            continue;
        }
        let f = dir.child(info.name());
        // `.desktop` launchers display their own Name=/Icon= entries
        // (localized) rather than the raw filename and the generic
        // x-desktop mimetype icon.
        let app = f
            .path()
            .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
            .and_then(gio::DesktopAppInfo::from_filename);
        items.push(Item {
            kind: Kind::File(f.clone()),
            name: app
                .as_ref()
                .map(|a| a.name().to_string())
                .unwrap_or_else(|| info.display_name().to_string()),
            icon: app.as_ref().and_then(|a| a.icon()).unwrap_or_else(|| {
                info.icon()
                    .unwrap_or_else(|| gio::ThemedIcon::new("text-x-generic").upcast())
            }),
            file: Some(f),
            mtime: info
                .modification_date_time()
                .map(|d| d.to_unix())
                .unwrap_or(0),
            size: info.size() as u64,
        });
    }
    items.sort_by(|a, b| cmp_items(a, b, by));
    items
}

/// Open an item with its default handler (or activate a mount).
pub fn open(item: &Item) {
    match &item.kind {
        Kind::File(f) => launch(f),
        _ => {
            if let Some(f) = &item.file {
                launch(f);
            }
        }
    }
}

/// Launch a file's default application. `.desktop` entries are
/// executed via `DesktopAppInfo` — opening them would just show
/// the file in a text editor.
fn launch(f: &gio::File) {
    if let Some(path) = f.path() {
        if path.extension().is_some_and(|e| e == "desktop") {
            if let Some(app) = gio::DesktopAppInfo::from_filename(&path) {
                match app.launch(&[], None::<&gio::AppLaunchContext>) {
                    Ok(()) => return,
                    Err(e) => eprintln!("[ragnar-view] launch {} failed: {e}", f.uri()),
                }
            }
        }
    }
    if let Err(e) = gio::AppInfo::launch_default_for_uri(&f.uri(), None::<&gio::AppLaunchContext>) {
        eprintln!("[ragnar-view] open {} failed: {e}", f.uri());
    }
}

/// Move an item to the trash.
pub fn trash(item: &Item) {
    if let Some(f) = &item.file {
        let _ = f.trash(None::<&gio::Cancellable>);
    }
}

/// A child of the Desktop dir named `base`, or `base (2)`, `base (3)`…
/// when that name is taken — creating never fails on a clash.
fn unique_child(base: &str) -> gio::File {
    let dir = desktop_dir();
    let mut f = dir.child(base);
    let mut n = 2;
    while f.query_exists(None::<&gio::Cancellable>) {
        f = dir.child(format!("{base} ({n})"));
        n += 1;
    }
    f
}

/// Create an empty document inside ~/Desktop; the created file so
/// the caller can place its icon where the menu was opened.
pub fn new_document(name: &str) -> Option<gio::File> {
    let f = unique_child(name);
    f.create(gio::FileCreateFlags::NONE, None::<&gio::Cancellable>)
        .ok()
        .map(|_| f)
}

/// Create a directory inside ~/Desktop; the created folder so the
/// caller can place its icon where the menu was opened.
pub fn new_folder(name: &str) -> Option<gio::File> {
    let f = unique_child(name);
    f.make_directory(None::<&gio::Cancellable>).ok().map(|_| f)
}

/// Delete a file or directory tree permanently. Symlinks are
/// removed, never followed.
fn delete_tree(f: &gio::File) -> Result<(), glib::Error> {
    let nofollow = gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS;
    let is_dir = f.query_file_type(nofollow, None::<&gio::Cancellable>) == gio::FileType::Directory;
    if is_dir {
        let en = f.enumerate_children("standard::name", nofollow, None::<&gio::Cancellable>)?;
        while let Some(info) = en.next_file(None::<&gio::Cancellable>)? {
            delete_tree(&f.child(info.name()))?;
        }
    }
    f.delete(None::<&gio::Cancellable>)
}

/// Permanently delete an item (files and directories on the desktop).
pub fn delete_permanently(item: &Item) {
    if let Some(f) = &item.file {
        if let Err(e) = delete_tree(f) {
            eprintln!("[ragnar-view] delete {} failed: {e}", f.uri());
        }
    }
}
