// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-osd — the overlay window
//
// A bottom-anchored gtk4-layer-shell surface, same geometry as the
// legacy OSD: 350 px wide, 80 px above the bottom edge, overlay layer,
// icon (48 px) + optional label + optional progress bar. Shows for
// 2500 ms, resets the timer on every new request. A single window
// instance is shared by the D-Bus surface and the built-in watchers.

use gio::prelude::*;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;

const OSD_WIDTH: i32 = 350;
const BOTTOM_MARGIN: i32 = 80;

thread_local! {
    /// The one OSD window for this process.
    static OSD: RefCell<Option<Rc<Osd>>> = const { RefCell::new(None) };
}

/// The overlay window plus its widgets and expire state.
struct Osd {
    window: gtk4::Window,
    icon: gtk4::Image,
    label: gtk4::Label,
    bar: gtk4::ProgressBar,
    expire: RefCell<Option<glib::SourceId>>,
}

/// Builds the window and registers it as the process-wide OSD.
pub fn init() {
    let icon = gtk4::Image::new();
    icon.set_pixel_size(48);

    let label = gtk4::Label::new(None);
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);

    let bar = gtk4::ProgressBar::new();
    bar.set_fraction(0.0);
    bar.set_hexpand(true);

    let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    text_col.set_valign(gtk4::Align::Center);
    text_col.set_hexpand(true);
    text_col.append(&label);
    text_col.append(&bar);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    row.set_margin_top(14);
    row.set_margin_bottom(14);
    row.set_margin_start(18);
    row.set_margin_end(18);
    row.append(&icon);
    row.append(&text_col);

    let window = gtk4::Window::new();
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_default_size(OSD_WIDTH, -1);
    window.set_child(Some(&row));

    // Layer-shell placement: overlay, bottom-anchored — identical to
    // the legacy OSD's GtkLayerShell setup.
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Bottom, BOTTOM_MARGIN);

    OSD.with(|o| {
        *o.borrow_mut() = Some(Rc::new(Osd {
            window,
            icon,
            label,
            bar,
            expire: RefCell::new(None),
        }))
    });
}

/// Shows the OSD — the shared entry point for the D-Bus Show() call
/// and the built-in volume/brightness watchers.
///
/// `icon`: freedesktop icon name (None hides the icon)
/// `label`: optional text line (markup allowed, like the legacy code)
/// `level`: progress 0.0..=1.0, None hides the bar
pub fn show(icon: Option<&str>, label: Option<&str>, level: Option<f64>) {
    // Master switch from settings.ini — disabled means silent.
    if !crate::config::enabled() {
        return;
    }
    let scale = crate::config::scale() as i32;
    OSD.with(|o| {
        let Some(osd) = o.borrow().clone() else {
            return;
        };
        // Scale knob: width and icon size follow the percent value.
        osd.window.set_default_size(OSD_WIDTH * scale / 100, -1);
        osd.icon.set_pixel_size(48 * scale / 100);
        match icon {
            Some(name) => {
                osd.icon.set_icon_name(Some(name));
                osd.icon.set_visible(true);
            }
            None => osd.icon.set_visible(false),
        }
        match label {
            Some(text) => {
                osd.label.set_markup(text);
                osd.label.set_visible(true);
            }
            None => osd.label.set_visible(false),
        }
        match level {
            Some(v) => {
                osd.bar.set_fraction(v.clamp(0.0, 1.0));
                osd.bar.set_visible(true);
            }
            None => osd.bar.set_visible(false),
        }
        // No explicit monitor: without an output binding the
        // compositor maps the surface to the focused output — same
        // effect as the legacy move_osd() picking a monitor.
        osd.window.present();
        arm_expire(&osd);
    });
}

/// Arms the 2.5 s auto-hide, replacing any pending timer — a new
/// event while visible just prolongs visibility.
fn arm_expire(osd: &Rc<Osd>) {
    if let Some(id) = osd.expire.borrow_mut().take() {
        id.remove();
    }
    let osd2 = osd.clone();
    let id = glib::timeout_add_local_once(
        std::time::Duration::from_millis(crate::config::duration_ms() as u64),
        move || {
            osd2.window.set_visible(false);
            // The source is already spent at this point — clear the
            // stored id so the next show() never tries to remove it.
            osd2.expire.borrow_mut().take();
        },
    );
    osd.expire.borrow_mut().replace(id);
}
