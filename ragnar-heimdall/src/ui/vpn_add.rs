//! The "import VPN profile" flow of the VPN page: a GTK file dialog
//! picks the provider config, an entry row asks for the profile
//! name, and the result goes back via `submit(name, path)`.

use std::rc::Rc;

use libadwaita as adw;

use adw::prelude::*;
use adw::{AlertDialog, EntryRow};
use gtk4::{gio, FileDialog, FileFilter, ListBox};

use crate::backend::VpnKind;

/// File type filter per VPN kind.
fn filter_for(kind: VpnKind) -> FileFilter {
    let filter = FileFilter::new();
    match kind {
        VpnKind::WireGuard => {
            filter.set_name(Some("WireGuard (*.conf)"));
            filter.add_pattern("*.conf");
        }
        VpnKind::OpenVpn => {
            filter.set_name(Some("OpenVPN (*.ovpn, *.conf)"));
            filter.add_pattern("*.ovpn");
            filter.add_pattern("*.conf");
        }
    }
    filter
}

/// Present a file picker on `parent`; on selection the name dialog
/// follows, then `submit` gets `(profile_name, file_path)`.
pub fn present<F: Fn(String, std::path::PathBuf) + 'static>(
    parent: &impl IsA<gtk4::Widget>,
    kind: VpnKind,
    submit: F,
) {
    let dialog = FileDialog::builder()
        .title(format!("{} {}", gettextrs::gettext("Import"), kind.label()))
        .default_filter(&filter_for(kind))
        .build();

    // The file dialog's callback outlives this function — capture an
    // owned (refcounted) clone of the parent widget.
    let parent_widget: gtk4::Widget = parent.upcast_ref().clone();
    let submit = Rc::new(submit);
    dialog.open(
        parent
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
            .as_ref(),
        None::<&gio::Cancellable>,
        move |result| {
            let Ok(file) = result else {
                return; // user cancelled
            };
            let Some(path) = file.path() else {
                return;
            };
            // Default name: the config file's stem — users recognize
            // what the provider called it.
            let default_name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            name_dialog(&parent_widget, kind, default_name, path, Rc::clone(&submit));
        },
    );
}

/// Second step: ask for the profile name before staging the file.
fn name_dialog<F: Fn(String, std::path::PathBuf) + 'static>(
    parent: &impl IsA<gtk4::Widget>,
    kind: VpnKind,
    default_name: String,
    path: std::path::PathBuf,
    submit: Rc<F>,
) {
    let dialog = AlertDialog::builder()
        .heading(format!(
            "{} {}",
            gettextrs::gettext("Name the"),
            kind.label()
        ))
        .body(gettextrs::gettext(
            "Shown in the profile list and used as the interface name.",
        ))
        .build();

    let list = ListBox::new();
    list.add_css_class("boxed-list");
    let entry = EntryRow::builder()
        .title(gettextrs::gettext("Profile name"))
        .build();
    entry.set_text(&default_name);
    list.append(&entry);
    dialog.set_extra_child(Some(&list));

    dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
    dialog.add_response("import", &gettextrs::gettext("Import"));
    dialog.set_response_appearance("import", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("import"));
    dialog.set_close_response("cancel");
    dialog.set_response_enabled("import", !default_name.trim().is_empty());

    {
        let dialog = dialog.clone();
        entry.connect_changed(move |e| {
            let text = e.text();
            let ok = !text.trim().is_empty()
                && text
                    .trim()
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
                && !text.trim().starts_with('.');
            dialog.set_response_enabled("import", ok);
        });
    }

    dialog.connect_response(None, move |_, response| {
        if response != "import" {
            return;
        }
        let name = entry.text().trim().to_string();
        if !name.is_empty() {
            submit(name, path.clone());
        }
    });
    dialog.present(Some(parent));
}
