// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-power-dialog — the chooser window
//
// A borderless-feeling adwaita window: heading, a flow of large
// action tiles (icon + label), and a small cancel button. Two CSS
// providers style it:
//   1. the compiled-in default (data/power-dialog.css)
//   2. ~/.config/ragnar-desktop/power-dialog.css — the user copy
//      loaded at USER priority, which is how Ragnar Settings →
//      Appearance (or a manual edit) re-themes the dialog later.
// Every styled element carries a stable class name — see the
// shipped power-dialog.css for the documented hooks.

use crate::{actions, i18n::tr};
use gtk4::{gdk, gio, glib, CssProvider};
use libadwaita as adw;
use libadwaita::prelude::*;

/// Path to the user theme, relative to the XDG config dir.
const USER_CSS: &str = "ragnar-desktop/power-dialog.css";

/// Build and show the dialog on the given application.
pub fn present(app: &adw::Application) {
    load_css();

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(tr("Sitzung"))
        .default_width(480)
        .resizable(false)
        .build();
    win.add_css_class("power-dialog");

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        content.set_property(&format!("margin-{m}"), 24);
    }

    let heading = gtk4::Label::new(Some(&tr("Was möchtest du tun?")));
    heading.add_css_class("title-2");
    content.append(&heading);

    // Action tiles — FlowBox centres the row no matter how many
    // actions the settings/capability filters left visible.
    let flow = gtk4::FlowBox::new();
    flow.set_selection_mode(gtk4::SelectionMode::None);
    flow.set_max_children_per_line(3);
    flow.set_min_children_per_line(3);
    flow.set_homogeneous(true);
    flow.set_column_spacing(12);
    flow.set_row_spacing(12);
    for action in actions::available() {
        flow.insert(&action_tile(action, &win), -1);
    }
    content.append(&flow);

    let cancel = gtk4::Button::with_label(&tr("Abbrechen"));
    cancel.add_css_class("power-dialog-cancel");
    cancel.add_css_class("pill");
    cancel.set_halign(gtk4::Align::Center);
    let w = win.clone();
    cancel.connect_clicked(move |_| w.close());
    content.append(&cancel);

    win.set_content(Some(&content));

    // Escape closes the dialog without doing anything.
    let esc = gtk4::EventControllerKey::new();
    let w = win.clone();
    esc.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            w.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    win.add_controller(esc);

    crate::daemon::track(&win);
    win.present();
}

/// One large tile: icon over label, executing the action on click.
fn action_tile(action: &'static actions::Action, win: &adw::ApplicationWindow) -> gtk4::Widget {
    let btn = gtk4::Button::new();
    btn.add_css_class("power-action");
    if action.destructive {
        btn.add_css_class("power-action-destructive");
    }

    let v = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    v.set_valign(gtk4::Align::Center);
    let icon = gtk4::Image::from_icon_name(action.icon);
    icon.add_css_class("power-action-icon");
    v.append(&icon);
    let label = gtk4::Label::new(Some(&actions::label(action)));
    label.add_css_class("power-action-label");
    label.set_wrap(true);
    v.append(&label);
    btn.set_child(Some(&v));

    let w = win.clone();
    btn.connect_clicked(move |_| {
        if !crate::daemon::handle_action(action.id) {
            actions::run(action.id);
        }
        w.close();
    });
    btn.upcast()
}

/// Install the two providers: compiled-in default first, then the
/// user file (if present) at a higher priority so it wins per rule.
fn load_css() {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let default = CssProvider::new();
    #[allow(deprecated)] // load_from_string/load_from_bytes need gtk4-rs ≥ 0.10
    default.load_from_data(include_str!("../data/power-dialog.css"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &default,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let user_file = dirs::config_dir().map(|d| d.join(USER_CSS));
    if let Some(path) = user_file.filter(|p| p.is_file()) {
        let user = CssProvider::new();
        user.load_from_file(&gio::File::for_path(path));
        gtk4::style_context_add_provider_for_display(
            &display,
            &user,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}
