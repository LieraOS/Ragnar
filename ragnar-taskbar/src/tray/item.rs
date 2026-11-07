// StatusNotifierItem property access. Items live at bus + object path;
// KDE-style ones sit at /StatusNotifierItem, ayatana-style under
// /org/ayatana/NotificationItem/<name>. When the stock path fails we
// introspect the owner's bus objects to locate the real one.
use crate::tray::Tray;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

type HashMapProps = std::collections::HashMap<String, glib::Variant>;

const IFACES: &[&str] = &["org.kde.StatusNotifierItem", "org.ayatana.NotificationItem"];

/// Fetch the item's properties: try every known interface at the
/// given path, then fall back to an introspection walk when neither
/// answers.
/// Retry bookkeeping for the registration race: some apps register
/// at the watcher before their properties exist, return an empty
/// GetAll once, and never emit NewIcon again. Without a retry those
/// items stay blank and dead forever — so we re-fetch a few times.
static RETRIES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u32>>> =
    std::sync::OnceLock::new();

fn retries() -> &'static std::sync::Mutex<std::collections::HashMap<String, u32>> {
    RETRIES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Whether the applied props still describe an icon-less item —
/// meaning the app answered but wasn't ready yet.
fn still_blank(tray: &Tray, key: &str) -> bool {
    tray.items
        .lock()
        .unwrap()
        .iter()
        .find(|i| i.key == key)
        .map(|i| i.icon_name.is_empty() && i.icon_pixmap.is_none())
        .unwrap_or(false)
}

/// Schedule one delayed re-fetch, capped at twelve attempts per
/// item (~18 s window — blueman-tray publishes its SNI object well
/// after claiming its bus name during session start).
fn schedule_retry(
    conn: &gio::DBusConnection,
    tray: &Tray,
    tx: &async_channel::Sender<()>,
    bus: String,
    path: String,
) {
    let key = format!("{bus}:{path}");
    let n = {
        let mut m = retries().lock().unwrap();
        let n = m.entry(key).or_insert(0);
        *n += 1;
        *n
    };
    if n > 12 {
        return;
    }
    let (c, t, x) = (conn.clone(), tray.clone(), tx.clone());
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        fetch(&c, &t, &x, bus, path);
    });
}

pub fn fetch(
    conn: &gio::DBusConnection,
    tray: &Tray,
    tx: &async_channel::Sender<()>,
    bus: String,
    path: String,
) {
    fetch_chain(conn.clone(), tray.clone(), tx.clone(), bus, path, 0);
}

/// Try IFACES[idx] at the known path; when all are exhausted, start
/// the introspection walk.
fn fetch_chain(
    conn: gio::DBusConnection,
    tray: Tray,
    tx: async_channel::Sender<()>,
    bus: String,
    path: String,
    idx: usize,
) {
    if idx >= IFACES.len() {
        discover_start(conn, tray, tx, bus, path);
        return;
    }
    let key0 = format!("{bus}:{path}");
    let iface = IFACES[idx];
    let (t2, x2) = (tray.clone(), tx.clone());
    let (b2, p2, c2) = (bus.clone(), path.clone(), conn.clone());
    let (conn_a, ba, pa) = (conn, bus, path);
    try_get_all(&conn_a, &ba, &pa, iface, move |props| {
        if let Some(props) = props {
            apply(&t2, &key0, props);
            let _ = x2.try_send(());
            // App answered but reported no icon data yet → schedule a
            // bounded re-fetch instead of leaving a dead blank slot.
            if still_blank(&t2, &key0) {
                schedule_retry(&c2, &t2, &x2, b2.clone(), p2.clone());
            }
        } else {
            fetch_chain(
                c2.clone(),
                t2.clone(),
                x2.clone(),
                b2.clone(),
                p2.clone(),
                idx + 1,
            );
        }
    });
}

/// Both stock interfaces failed — walk the owner's object tree for a
/// node exposing an SNI/ayatana interface, then fetch there.
fn discover_start(
    conn: gio::DBusConnection,
    tray: Tray,
    tx: async_channel::Sender<()>,
    bus: String,
    path: String,
) {
    let key0 = format!("{bus}:{path}");
    let (t2, x2) = (tray.clone(), tx.clone());
    let bus2 = bus.clone();
    let conn_d = conn.clone();
    let conn_r = conn.clone();
    discover(&conn_d, &bus, "/", 0, move |found| {
        if let Some((path, iface)) = found {
            // Point the table entry at the real object so Activate
            // reaches it too.
            {
                let mut items = t2.items.lock().unwrap();
                if let Some(it) = items.iter_mut().find(|i| i.key == key0) {
                    it.path = path.clone();
                }
            }
            let k3 = key0.clone();
            let conn3 = conn.clone();
            let (t4, x4, b4, p4, c4) = (
                t2.clone(),
                x2.clone(),
                bus2.clone(),
                path.clone(),
                conn_r.clone(),
            );
            try_get_all(&conn3, &bus2, &path, &iface, move |props| {
                if let Some(props) = props {
                    apply(&t4, &k3, props);
                }
                // Re-render with the discovered icon — the outer
                // notify already fired before this fetch returns.
                let _ = x4.try_send(());
                // Props arrived but still no icon — keep retrying.
                if still_blank(&t4, &k3) {
                    schedule_retry(&c4, &t4, &x4, b4.clone(), p4.clone());
                }
            });
        } else {
            // No SNI object anywhere on the owner yet — the app has
            // not published it; retry instead of a permanent blank.
            schedule_retry(&conn_r, &t2, &x2, bus2.clone(), path.clone());
        }
        let _ = x2.try_send(()); // notify anyway — fallback icon
    });
}

/// Properties.GetAll on one iface — Some(dict) or None on failure.
fn try_get_all(
    conn: &gio::DBusConnection,
    bus: &str,
    path: &str,
    iface: &str,
    done: impl Fn(Option<HashMapProps>) + 'static,
) {
    let done = std::rc::Rc::new(done);
    let (bus, path, iface) = (bus.to_string(), path.to_string(), iface.to_string());
    conn.call(
        Some(&bus),
        &path,
        "org.freedesktop.DBus.Properties",
        "GetAll",
        // D-Bus params must always be a tuple, even for one arg.
        Some(&(iface.to_string(),).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            let props = res
                .ok()
                .and_then(|v| v.get::<(HashMapProps,)>())
                .map(|t| t.0);
            done(props);
        },
    );
}

/// Introspect exactly one node: returns (exposes_sni, child names).
fn introspect(
    conn: &gio::DBusConnection,
    bus: &str,
    path: &str,
    done: impl Fn(Option<(Option<String>, Vec<String>)>) + 'static,
) {
    let done = std::rc::Rc::new(done);
    let (bus2, path2) = (bus.to_string(), path.to_string());
    conn.call(
        Some(bus),
        path,
        "org.freedesktop.DBus.Introspectable",
        "Introspect",
        None,
        None,
        gio::DBusCallFlags::NONE,
        3000,
        None::<&gio::Cancellable>,
        move |res| {
            let Ok(v) = res else {
                done(None);
                return;
            };
            let xml = v.child_value(0).str().unwrap_or("").to_string();
            let hit = IFACES
                .iter()
                .find(|i| xml.contains(**i))
                .map(|s| s.to_string());
            let children: Vec<String> = xml
                .split("<node name=\"")
                .skip(1)
                .filter_map(|s| s.split('"').next().map(|n| n.to_string()))
                .filter(|n| !n.is_empty())
                .collect();
            let _ = (bus2, path2);
            done(Some((hit, children)));
        },
    );
}

/// Depth-limited walk (0=root, max 4) until a node exposes an SNI
/// interface; each step queues its children onto the shared stack.
fn discover(
    conn: &gio::DBusConnection,
    bus: &str,
    path: &str,
    depth: u8,
    done: impl Fn(Option<(String, String)>) + 'static,
) {
    let stack = std::rc::Rc::new(std::cell::RefCell::new(vec![(path.to_string(), depth)]));
    let done = std::rc::Rc::new(done);
    step(conn, bus, stack, done);
}

/// The discovery callback — `None` when the walk found no SNI object.
type Done = std::rc::Rc<dyn Fn(Option<(String, String)>)>;

fn step(
    conn: &gio::DBusConnection,
    bus: &str,
    stack: std::rc::Rc<std::cell::RefCell<Vec<(String, u8)>>>,
    done: Done,
) {
    let next = stack.borrow_mut().pop();
    let Some((path, depth)) = next else {
        done(None);
        return;
    };
    let (conn2, bus2) = (conn.clone(), bus.to_string());
    let path_a = path.clone();
    introspect(conn, bus, &path_a, move |res| {
        let Some((hit, children)) = res else {
            step(&conn2, &bus2, stack.clone(), done.clone());
            return;
        };
        if let Some(iface) = hit {
            done(Some((path.clone(), iface)));
            return;
        }
        if depth < 4 {
            let mut st = stack.borrow_mut();
            for c in children {
                st.push((format!("{path}/{c}").replace("//", "/"), depth + 1));
            }
            drop(st);
        }
        step(&conn2, &bus2, stack.clone(), done.clone());
    });
}

/// Apply a fetched property set to the matching table entry.
fn apply(tray: &Tray, key: &str, props: HashMapProps) {
    let mut icon_name = String::new();
    let mut theme_path = String::new();
    let mut pixmap = None;
    let mut menu_path = String::new();
    let mut status = "Active".to_string();
    for (k, val) in props.iter() {
        match k.as_str() {
            "IconName" => {
                if let Some(s) = val.get::<String>() {
                    icon_name = s;
                }
            }
            "Status" => {
                if let Some(s) = val.get::<String>() {
                    status = s;
                }
            }
            "IconThemePath" => {
                if let Some(s) = val.get::<String>() {
                    theme_path = s;
                }
            }
            "Menu" => {
                if let Some(s) = val.get::<String>() {
                    menu_path = s;
                }
            }
            "IconPixmap" => pixmap = parse_pixmap(val),
            _ => {}
        }
    }
    let mut items = tray.items.lock().unwrap();
    if let Some(it) = items.iter_mut().find(|i| i.key == key) {
        it.icon_name = icon_name;
        it.icon_theme_path = theme_path;
        it.menu_path = menu_path;
        it.icon_pixmap = pixmap;
        it.status = status;
    }
}

/// KDE IconPixmap: a[(width, height, ARGB32 network-order bytes)] —
/// take the largest frame; callers convert to a texture.
fn parse_pixmap(v: &glib::Variant) -> Option<(i32, i32, Vec<u8>)> {
    let frames = v.get::<Vec<(i32, i32, Vec<u8>)>>()?;
    frames.into_iter().max_by_key(|(w, _, _)| *w)
}
