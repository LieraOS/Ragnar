// com.canonical.dbusmenu client session. SNI items expose a `Menu`
// property naming a D-Bus object implementing this interface; menus
// arrive as nested layout tuples, clicks go back as Event calls, and
// LayoutUpdated signals rebuild the open menu in place.
use super::dbusmenu_parse::parse_tree;
use super::dbusmenu_ui::{fill, MenuCtx};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

/// Fetch the item's menu and show it above the clicked tray icon.
/// AboutToShow runs first per spec — services may build the layout
/// lazily and only then answer GetLayout correctly.
pub fn open_menu(
    conn: &gio::DBusConnection,
    bus: &str,
    menu_path: &str,
    parent: &impl IsA<gtk4::Widget>,
    _x: f64,
    _y: f64,
) {
    let (bus, path) = (bus.to_string(), menu_path.to_string());
    let parent = parent.clone().upcast::<gtk4::Widget>();
    // The call borrows bus/path only for the duration of the send —
    // the clones below move into the reply closure instead.
    let (conn_a, bus_a, path_a) = (conn.clone(), bus.clone(), path.clone());
    conn.call(
        Some(bus.as_str()),
        path.as_str(),
        "com.canonical.dbusmenu",
        "AboutToShow",
        Some(&(0i32,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        2000,
        None::<&gio::Cancellable>,
        move |_| fetch_layout(&conn_a, &bus_a, &path_a, &parent),
    );
}

/// GetLayout(0, -1, []) → build the menu on the overlay shell and
/// arm the live-update subscription for this menu session.
fn fetch_layout(conn: &gio::DBusConnection, bus: &str, path: &str, parent: &gtk4::Widget) {
    let (conn2, bus2, path2, parent2) = (
        conn.clone(),
        bus.to_string(),
        path.to_string(),
        parent.clone(),
    );
    conn.call(
        Some(bus),
        path,
        "com.canonical.dbusmenu",
        "GetLayout",
        Some(&(0i32, -1i32, Vec::<String>::new()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            let Ok(v) = res else { return };
            let items = parse_tree(&v.child_value(1));
            show(&conn2, &bus2, &path2, items, &parent2);
        },
    );
}

/// Open the overlay menu with the parsed items and subscribe to
/// LayoutUpdated so the service can rebuild the open menu live.
fn show(
    conn: &gio::DBusConnection,
    bus: &str,
    path: &str,
    items: Vec<super::dbusmenu_parse::MenuItem>,
    parent: &gtk4::Widget,
) {
    let (lft, rgt, sw, h) = crate::menu::widget_span(parent);
    let win = crate::menu::shell(lft, rgt, sw, h);
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let ctx = MenuCtx {
        conn: conn.clone(),
        bus: bus.to_string(),
        path: path.to_string(),
    };
    fill(&list, &ctx, &items);
    crate::menu::show(&win, &list);
    subscribe_layout(conn, bus, path, &win);
}

/// Rebuild the open menu when the service signals LayoutUpdated —
/// refetches the layout and swaps the menu box content in place.
/// The subscription dies with the window.
fn subscribe_layout(conn: &gio::DBusConnection, bus: &str, path: &str, win: &gtk4::Window) {
    let w = win.downgrade();
    let (c, b, p) = (conn.clone(), bus.to_string(), path.to_string());
    let sub_id = conn.signal_subscribe(
        Some(bus),
        Some("com.canonical.dbusmenu"),
        Some("LayoutUpdated"),
        Some(path),
        None,
        gio::DBusSignalFlags::NONE,
        move |_c, _s, _p, _i, _sig, _params| {
            let Some(win) = w.upgrade() else { return };
            refetch_into(&c, &b, &p, &win);
        },
    );
    // Menu window gone → stop listening; a later open re-subscribes.
    // SignalSubscriptionId isn't Copy — keep it in a Cell so the Fn
    // closure can move it out exactly once.
    let sub = std::cell::Cell::new(Some(sub_id));
    let c2 = conn.clone();
    win.connect_destroy(move |_| {
        if let Some(id) = sub.take() {
            c2.signal_unsubscribe(id);
        }
    });
}

/// GetLayout for a live rebuild: replaces the root list's children
/// and drops any open cascade boxes.
fn refetch_into(conn: &gio::DBusConnection, bus: &str, path: &str, win: &gtk4::Window) {
    let (c, b, p, w) = (conn.clone(), bus.to_string(), path.to_string(), win.clone());
    conn.call(
        Some(bus),
        path,
        "com.canonical.dbusmenu",
        "GetLayout",
        Some(&(0i32, -1i32, Vec::<String>::new()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            let Ok(v) = res else { return };
            rebuild(&c, &b, &p, &w, &v.child_value(1));
        },
    );
}

/// Swap the open menu's content for a fresh layout: remove cascade
/// boxes from the overlay, clear the root list, refill both.
fn rebuild(
    conn: &gio::DBusConnection,
    bus: &str,
    path: &str,
    win: &gtk4::Window,
    layout: &glib::Variant,
) {
    let items = parse_tree(layout);
    let menu_box =
        unsafe { win.data::<gtk4::Box>("menu_box") }.map(|p| unsafe { p.as_ref() }.clone());
    let overlay =
        unsafe { win.data::<gtk4::Overlay>("overlay") }.map(|p| unsafe { p.as_ref() }.clone());
    let boxes =
        unsafe { win.data::<std::rc::Rc<std::cell::RefCell<Vec<gtk4::Box>>>>("menu_boxes") }
            .map(|p| unsafe { p.as_ref() }.clone());
    let (Some(menu_box), Some(overlay), Some(boxes)) = (menu_box, overlay, boxes) else {
        return;
    };
    // Drop cascade levels — the overlay keeps only its main child.
    {
        let mut b = boxes.borrow_mut();
        while b.len() > 1 {
            if let Some(sub) = b.pop() {
                overlay.remove_overlay(&sub);
            }
        }
    }
    while let Some(c) = menu_box.first_child() {
        menu_box.remove(&c);
    }
    let list = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let ctx = MenuCtx {
        conn: conn.clone(),
        bus: bus.to_string(),
        path: path.to_string(),
    };
    fill(&list, &ctx, &items);
    menu_box.append(&list);
}

/// Deliver the click back to the item's menu service.
pub fn send_event(conn: &gio::DBusConnection, bus: &str, path: &str, id: i32) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    // The 'v' slot takes any variant — an empty string suffices.
    let data = "".to_variant();
    // Async — the click must not wait on the item's reply.
    let (conn, bus, path) = (conn.clone(), bus.to_string(), path.to_string());
    glib::spawn_future_local(async move {
        let _ = conn
            .call_future(
                Some(&bus),
                &path,
                "com.canonical.dbusmenu",
                "Event",
                Some(&(id, "clicked", data, ts).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2000,
            )
            .await;
    });
}
