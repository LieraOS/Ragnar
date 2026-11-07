// dbusmenu layout → GTK widgets. Rows carry icon + label, toggles
// render as check buttons, separators as dividers, and submenu rows
// cascade into a second menu box on the same overlay surface
// (menu::submenu) — real submenus, not indentation.
use super::dbusmenu_parse::MenuItem;
use gtk4::gio;
use gtk4::prelude::*;

/// Shared per-menu session handles every row's click needs.
#[derive(Clone)]
pub struct MenuCtx {
    pub conn: gio::DBusConnection,
    pub bus: String,
    pub path: String,
}

/// Render the item list into `box_`. `depth` exists so submenu rows
/// inside a cascade close the whole menu chain after leaf clicks —
// identical to root behaviour, but kept explicit.
pub fn fill(box_: &gtk4::Box, ctx: &MenuCtx, items: &[MenuItem]) {
    for item in items {
        if !item.visible {
            continue;
        }
        if item.kind == "separator" {
            box_.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
            continue;
        }
        let id = item.id;
        match item.toggle_type.as_str() {
            // Toggle item: CheckButton reflects toggle-state and
            // sends "clicked" on flip — the service owns grouping
            // semantics for radio entries.
            "checkmark" | "radio" if !item.has_submenu => {
                let cb = gtk4::CheckButton::with_label(&item.label);
                cb.set_active(item.toggle_state == 1);
                cb.set_sensitive(item.enabled);
                let c = ctx.clone();
                cb.connect_toggled(move |_| {
                    super::dbusmenu::send_event(&c.conn, &c.bus, &c.path, id);
                    crate::menu::close_current();
                });
                box_.append(&cb);
            }
            _ => {
                let row = plain_row(item);
                row.set_sensitive(item.enabled);
                if item.has_submenu {
                    // Click cascades the children into a sibling menu
                    // box anchored at this row.
                    let (c, kids, r) = (ctx.clone(), item.children.clone(), row.clone());
                    row.connect_clicked(move |_| {
                        if let Some(sub) = crate::menu::submenu(&r) {
                            fill(&sub, &c, &kids);
                        }
                    });
                } else {
                    let c = ctx.clone();
                    row.connect_clicked(move |_| {
                        super::dbusmenu::send_event(&c.conn, &c.bus, &c.path, id);
                        crate::menu::close_current();
                    });
                }
                box_.append(&row);
            }
        }
    }
}

/// Standard item: optional icon + label + submenu arrow.
fn plain_row(item: &MenuItem) -> gtk4::Button {
    let inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    if let Some(img) = icon(item) {
        inner.append(&img);
    }
    let label = gtk4::Label::new(Some(&item.label));
    label.set_halign(gtk4::Align::Start);
    label.set_hexpand(true);
    inner.append(&label);
    if item.has_submenu {
        inner.append(&gtk4::Label::new(Some("›")));
    }
    let btn = gtk4::Button::new();
    btn.set_child(Some(&inner));
    btn.set_has_frame(false);
    btn.set_halign(gtk4::Align::Fill);
    btn
}

/// Item icon: freedesktop name via theme; icon-data is PNG bytes or
// raw ARGB32 — try the image decoders first, then raw square pixels.
fn icon(item: &MenuItem) -> Option<gtk4::Image> {
    if !item.icon_name.is_empty() {
        let img = gtk4::Image::from_icon_name(&item.icon_name);
        img.set_pixel_size(16);
        return Some(img);
    }
    let data = item.icon_data.as_ref()?;
    let bytes = gtk4::glib::Bytes::from(data);
    // gdk decodes PNG/JPEG from memory — the common icon-data case.
    if let Ok(tex) = gtk4::gdk::Texture::from_bytes(&bytes) {
        let img = gtk4::Image::from_paintable(Some(&tex));
        img.set_pixel_size(16);
        return Some(img);
    }
    // Raw ARGB32 (network order A,R,G,B) without dimensions — only
    // decodable when the payload is a perfect square of 4-byte px.
    let px = data.len() / 4;
    let side = (px as f64).sqrt() as usize;
    if side == 0 || side * side != px {
        return None;
    }
    let mut rgba = Vec::with_capacity(data.len());
    for c in data.chunks_exact(4) {
        rgba.extend_from_slice(&[c[1], c[2], c[3], c[0]]);
    }
    let tex = gtk4::gdk::MemoryTexture::new(
        side as i32,
        side as i32,
        gtk4::gdk::MemoryFormat::R8g8b8a8,
        &gtk4::glib::Bytes::from(&rgba),
        side * 4,
    );
    let img = gtk4::Image::from_paintable(Some(&tex));
    img.set_pixel_size(16);
    Some(img)
}
