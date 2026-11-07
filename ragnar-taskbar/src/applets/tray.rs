// System-tray applet — one icon button per registered
// StatusNotifierItem (Discord, Steam, Blueman, …). Icons come from
// the item's IconName via the theme, or from its ARGB32 pixmap
// converted to a texture. Click calls the item's Activate method;
// Passive items stay hidden like other trays do.
use crate::tray::{Tray, TrayItem};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

/// The live cell the single consumer renders into — re-registered on
/// every layout rebuild so one consumer survives bar reconfiguration.
pub type TrayZone = std::rc::Rc<std::cell::RefCell<Option<gtk4::Box>>>;

/// Start the ONE notify consumer for a bar. Must be called exactly
/// once per bar — a second consumer would split the notify stream and
/// the tray would starve (the bug where icons vanished on settings).
pub fn start(tray: &Tray, rx: async_channel::Receiver<()>, size: i32) -> TrayZone {
    let zone: TrayZone = std::rc::Rc::new(std::cell::RefCell::new(None));
    let (t, tg, sz) = (tray.clone(), zone.clone(), size);
    glib::spawn_future_local(async move {
        while rx.recv().await.is_ok() {
            if let Some(z) = tg.borrow().as_ref() {
                rebuild(z, &t, sz);
            }
        }
    });
    zone
}

/// Create the tray zone widget for the current layout, register it as
/// the render target and repopulate from the shared item table.
pub fn build(tray: &Tray, zone: &TrayZone, size: i32, vertical: bool) -> gtk4::Widget {
    let z = gtk4::Box::new(
        if vertical {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        },
        4,
    );
    *zone.borrow_mut() = Some(z.clone());
    rebuild(&z, tray, size);
    z.upcast()
}

/// Swap the icon set for the current item table — Active items only.
fn rebuild(zone: &gtk4::Box, tray: &Tray, size: i32) {
    while let Some(c) = zone.first_child() {
        zone.remove(&c);
    }
    let snap: Vec<TrayItem> = tray.items.lock().unwrap().clone();
    for item in snap.iter() {
        if item.status == "Passive" {
            continue;
        }
        let button = gtk4::Button::new();
        button.set_child(Some(&icon_for(item, size)));
        button.add_css_class("tray-button");
        let (bus, path) = (item.sender.clone(), item.path.clone());
        button.connect_clicked(move |_| activate(&bus, &path));
        if !item.menu_path.is_empty() {
            let click = gtk4::GestureClick::new();
            click.set_button(3);
            let (bus, mpath, btn) = (item.sender.clone(), item.menu_path.clone(), button.clone());
            click.connect_pressed(move |g, _, x, y| {
                let _ = g;
                // Async bus connect — never block the click handler.
                let (bus2, mpath2, btn2) = (bus.clone(), mpath.clone(), btn.clone());
                glib::spawn_future_local(async move {
                    if let Ok(conn) = gtk4::gio::bus_get_future(gtk4::gio::BusType::Session).await {
                        crate::tray::dbusmenu::open_menu(&conn, &bus2, &mpath2, &btn2, x, y);
                    }
                });
            });
            button.add_controller(click);
        }
        zone.append(&button);
    }
}

/// Resolve the item icon: theme name first, embedded pixmap as
/// fallback — same rule other trays apply.
fn icon_for(item: &TrayItem, size: i32) -> gtk4::Image {
    let theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());
    // Items may carry a private icon dir (IconThemePath) — extend the
    // theme search path so custom names like "status_icon_0" resolve.
    if !item.icon_theme_path.is_empty() {
        let dir = std::path::PathBuf::from(&item.icon_theme_path);
        let mut paths = theme.search_path();
        if !paths.contains(&dir) {
            paths.push(dir);
            let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
            theme.set_search_path(&refs);
        }
    }
    // Items may carry a private icon dir (IconThemePath): the file
    // <dir>/<IconName>.<ext> is the icon — load it directly, theme
    // resolution is unnecessary and unreliable for these.
    let mut themed = None;
    if !item.icon_theme_path.is_empty() && !item.icon_name.is_empty() {
        for ext in ["png", "svg", "xpm"] {
            let f = std::path::Path::new(&item.icon_theme_path)
                .join(format!("{}.{}", item.icon_name, ext));
            if f.exists() {
                themed = Some(gtk4::Image::from_file(&f));
                break;
            }
        }
    }
    let img = if let Some(img) = themed {
        img
    } else if item.icon_name.contains('/') {
        // ayatana-style items ship an absolute icon path, not a name.
        gtk4::Image::from_file(&item.icon_name)
    } else if !item.icon_name.is_empty() {
        // Try the bare name first, then the xsi- prefixed variant the
        // hicolor fallback theme ships — the configured theme may be
        // absent entirely, and then only hicolor names resolve.
        let cands = [item.icon_name.clone(), format!("xsi-{}", item.icon_name)];
        cands
            .iter()
            .find(|n| theme.has_icon(n.as_str()))
            .map(|n| gtk4::Image::from_icon_name(n))
            .unwrap_or_else(|| gtk4::Image::from_icon_name("image-missing"))
    } else if let Some((w, h, data)) = &item.icon_pixmap {
        pixmap_texture(*w, *h, data)
            .map(|tex| gtk4::Image::from_paintable(Some(&tex)))
            .unwrap_or_default()
    } else {
        // Always-visible fallback — never an empty slot.
        gtk4::Image::from_icon_name("image-missing")
    };
    img.set_pixel_size(size);
    img
}

/// KDE IconPixmap data is ARGB32 in network byte order (A,R,G,B) —
/// rearrange to RGBA for gdk.
fn pixmap_texture(w: i32, h: i32, data: &[u8]) -> Option<gdk::MemoryTexture> {
    if w <= 0 || h <= 0 || data.len() < (w * h * 4) as usize {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in data.chunks_exact(4).take((w * h) as usize) {
        rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
    }
    Some(gdk::MemoryTexture::new(
        w,
        h,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from(&rgba),
        (w * 4) as usize,
    ))
}

/// Ask the item to do its primary action — same contract as every
/// SNI host (Activate with screen coords; 0/0 is acceptable). Async:
/// a wedged item must not freeze the bar for the call timeout.
fn activate(bus: &str, path: &str) {
    let (bus, path) = (bus.to_string(), path.to_string());
    glib::spawn_future_local(async move {
        let Ok(conn) = gio::bus_get_future(gio::BusType::Session).await else {
            return;
        };
        let _ = conn
            .call_future(
                Some(&bus),
                &path,
                "org.kde.StatusNotifierItem",
                "Activate",
                Some(&(0i32, 0i32).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2000,
            )
            .await;
    });
}
