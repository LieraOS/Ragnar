// Launcher properties dialog ("Startereigenschaften") — the classic
// editor window: icon button, Name, Befehl + Durchsuchen, Kommentar,
// "Im Terminal ausführen" and "Dedizierte GPU" checkboxes, OK /
// Abbrechen. Used for editing existing apps (writes a per-user
// .desktop override) and for creating brand-new launchers.
use gtk4::gio;
use gtk4::prelude::*;
use std::rc::Rc;

/// Everything the dialog edits — mirrors [Desktop Entry] keys.
#[derive(Clone)]
pub struct Fields {
    /// Existing desktop id ("firefox.desktop"); None = new launcher.
    pub id: Option<String>,
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub icon: String,
    pub terminal: bool,
    pub gpu: bool,
}

/// Prefill from the resolved .desktop of an app id.
pub fn for_app(id: &str) -> Fields {
    let mut f = Fields {
        id: Some(id.to_string()),
        name: String::new(),
        exec: String::new(),
        comment: String::new(),
        icon: String::new(),
        terminal: false,
        gpu: false,
    };
    let Some(ai) = gio::DesktopAppInfo::new(id) else {
        return f;
    };
    let Some(p) = ai.filename() else {
        return f;
    };
    let kf = gtk4::glib::KeyFile::new();
    if kf
        .load_from_file(&p, gtk4::glib::KeyFileFlags::NONE)
        .is_err()
    {
        return f;
    }
    let get = |k: &str| {
        kf.string("Desktop Entry", k)
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    f.name = get("Name");
    f.exec = get("Exec");
    f.comment = get("Comment");
    f.icon = get("Icon");
    f.terminal = kf.boolean("Desktop Entry", "Terminal").unwrap_or(false);
    f.gpu = kf
        .boolean("Desktop Entry", "PrefersNonDefaultGPU")
        .unwrap_or(false);
    f
}

/// Path of the per-user override/new file for a desktop id.
fn user_file(id: &str) -> std::path::PathBuf {
    gtk4::glib::user_data_dir().join("applications").join(id)
}

/// Persist fields: clone the resolved file for existing ids and
/// rewrite the managed keys inside [Desktop Entry]; create a fresh
/// file for new launchers.
pub fn save(f: &Fields) {
    let id = match &f.id {
        Some(id) => id.clone(),
        None => new_id(&f.name),
    };
    let text = match &f.id {
        Some(id) => gio::DesktopAppInfo::new(id)
            .and_then(|ai| ai.filename())
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default(),
        None => String::new(),
    };
    let managed = [
        ("Name", f.name.as_str()),
        ("Comment", f.comment.as_str()),
        ("Exec", f.exec.as_str()),
        ("Icon", f.icon.as_str()),
        ("Terminal", if f.terminal { "true" } else { "false" }),
        ("PrefersNonDefaultGPU", if f.gpu { "true" } else { "false" }),
    ];
    let mut out = if text.is_empty() {
        "[Desktop Entry]\nType=Application\n".to_string()
    } else {
        String::new()
    };
    let mut in_entry = false;
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        if line.starts_with('[') {
            if in_entry {
                // End of [Desktop Entry] — append keys the source lacked.
                for (k, v) in managed.iter().filter(|(k, _)| !seen.contains(*k)) {
                    out.push_str(&format!("{k}={v}\n"));
                }
            }
            in_entry = line == "[Desktop Entry]";
        }
        let replaced = in_entry
            && managed
                .iter()
                .find(|(k, _)| line.starts_with(&format!("{k}=")))
                .map(|(k, v)| {
                    seen.insert(*k);
                    out.push_str(&format!("{k}={v}\n"));
                })
                .is_some();
        if !replaced && !line.trim().is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    if in_entry {
        for (k, v) in managed.iter().filter(|(k, _)| !seen.contains(*k)) {
            out.push_str(&format!("{k}={v}\n"));
        }
    }
    let dst = user_file(&id);
    let _ = std::fs::create_dir_all(dst.parent().unwrap());
    let _ = std::fs::write(&dst, out);
}

/// A freedesktop-conformant id for a user-created launcher.
fn new_id(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    format!("userapp-{}-{}.desktop", slug, std::process::id())
}

/// Open the properties dialog. `on_save` receives the edited fields
/// when the user hits OK.
pub fn open(parent: &gtk4::Window, f: Fields, on_save: Rc<dyn Fn(Fields)>) {
    let win = gtk4::Window::builder()
        .title(crate::i18n::tr("Startereigenschaften"))
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .resizable(false)
        .build();
    let grid = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    grid.set_margin_top(18);
    grid.set_margin_bottom(18);
    grid.set_margin_start(18);
    grid.set_margin_end(18);

    // Icon button (top-left, like the reference dialog) — click opens
    // a file picker and stores the chosen path in the icon field.
    let icon = gtk4::Image::new();
    icon.set_pixel_size(48);
    if !f.icon.is_empty() {
        if std::path::Path::new(&f.icon).is_absolute() {
            icon.set_from_file(Some(&f.icon));
        } else {
            icon.set_icon_name(Some(&f.icon));
        }
    } else {
        icon.set_icon_name(Some("application-x-executable"));
    }
    let icon_val = Rc::new(std::cell::RefCell::new(f.icon.clone()));
    let icon_btn = gtk4::Button::new();
    icon_btn.set_child(Some(&icon));
    icon_btn.set_tooltip_text(Some(&crate::i18n::tr("Symbol wählen")));
    {
        let (w, i, iv) = (win.clone(), icon.clone(), icon_val.clone());
        icon_btn.connect_clicked(move |_| {
            let (w2, i2, iv2) = (w.clone(), i.clone(), iv.clone());
            gtk4::FileDialog::new().open(Some(&w), gtk4::gio::Cancellable::NONE, move |r| {
                if let Ok(file) = r {
                    if let Some(path) = file.path() {
                        i2.set_from_file(Some(&path));
                        *iv2.borrow_mut() = path.to_string_lossy().to_string();
                    }
                }
                let _ = w2;
            });
        });
    }
    grid.append(&icon_btn);

    let labeled = |label: &str, entry: &gtk4::Entry| {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let l = gtk4::Label::new(Some(&crate::i18n::tr(label)));
        l.set_width_chars(12);
        l.set_xalign(1.0);
        row.append(&l);
        entry.set_hexpand(true);
        row.append(entry);
        row
    };

    let name = gtk4::Entry::builder().text(&f.name).build();
    grid.append(&labeled("Name:", &name));
    let exec = gtk4::Entry::builder().text(&f.exec).build();
    let exec_row = labeled("Befehl:", &exec);
    let browse = gtk4::Button::with_label(&crate::i18n::tr("Durchsuchen"));
    {
        let (w, e) = (win.clone(), exec.clone());
        browse.connect_clicked(move |_| {
            let e2 = e.clone();
            gtk4::FileDialog::new().open(Some(&w), gtk4::gio::Cancellable::NONE, move |r| {
                if let Ok(file) = r {
                    if let Some(path) = file.path() {
                        e2.set_text(&path.to_string_lossy());
                    }
                }
            });
        });
    }
    exec_row.append(&browse);
    grid.append(&exec_row);
    let comment = gtk4::Entry::builder().text(&f.comment).build();
    grid.append(&labeled("Kommentar:", &comment));

    let term = gtk4::CheckButton::with_label(&crate::i18n::tr("Im Terminal ausführen?"));
    term.set_active(f.terminal);
    grid.append(&term);
    let gpu = gtk4::CheckButton::with_label(&crate::i18n::tr(
        "Dedizierte GPU verwenden, falls verfügbar",
    ));
    gpu.set_active(f.gpu);
    grid.append(&gpu);

    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&crate::i18n::tr("Abbrechen"));
    let ok = gtk4::Button::with_label(&crate::i18n::tr("OK"));
    ok.add_css_class("suggested-action");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let (w, id) = (win.clone(), f.id.clone());
        ok.connect_clicked(move |_| {
            on_save(Fields {
                id: id.clone(),
                name: name.text().to_string(),
                exec: exec.text().to_string(),
                comment: comment.text().to_string(),
                icon: icon_val.borrow().clone(),
                terminal: term.is_active(),
                gpu: gpu.is_active(),
            });
            w.close();
        });
    }
    btns.append(&cancel);
    btns.append(&ok);
    grid.append(&btns);
    win.set_child(Some(&grid));
    win.present();
}
