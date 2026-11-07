// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-osd — the D-Bus surface
//
// Owns org.lieraos.ragnar.OSD at /org/lieraos/ragnar/OSD and serves
// Show(a{sv}) — the same contract the legacy OSDManager exposed, so
// callers (OSDKeys-equivalents, media-key handlers, any Ragnar
// component) work unchanged:
//
//   "icon"  -> s  freedesktop icon name
//   "label" -> s  text line (markup)
//   "level" -> d  progress 0.0..1.0  (gsd-doubles build)
//          or i/u progress 0..100   (int build) — accepted too
//   "monitor" -> s connector name to display on (optional, honoured
//              when the compositor reports one)

use crate::window;
use gio::prelude::*;

const NAME: &str = "org.lieraos.ragnar.OSD";
const PATH: &str = "/org/lieraos/ragnar/OSD";

const XML: &str = r#"<node>
  <interface name="org.lieraos.ragnar.OSD">
    <method name="Show"><arg type="a{sv}" direction="in"/></method>
  </interface>
</node>"#;

/// Owns the bus name; the OwnerId must be kept alive.
pub fn own_name() -> gio::OwnerId {
    gio::bus_own_name(
        gio::BusType::Session,
        NAME,
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| register(&conn),
        |_, _| {},
        |_, _| eprintln!("ragnar-osd: lost bus name {NAME}"),
    )
}

/// Registers the object and dispatches Show().
fn register(conn: &gio::DBusConnection) {
    let node = gio::DBusNodeInfo::for_xml(XML).expect("valid xml");
    let Some(info) = node.lookup_interface(NAME) else {
        return;
    };
    conn.register_object(PATH, &info)
        .method_call(|_, _, _, _, method, params, inv| {
            if method == "Show" {
                let dict = glib::VariantDict::new(Some(&params.child_value(0)));
                let icon = dict
                    .lookup_value("icon", None)
                    .and_then(|v| v.get::<String>());
                let label = dict
                    .lookup_value("label", None)
                    .and_then(|v| v.get::<String>());
                let level = dict.lookup_value("level", None).and_then(normalize_level);
                window::show(icon.as_deref(), label.as_deref(), level);
            }
            inv.return_value(None);
        })
        .build();
}

/// Normalizes the level argument: double in 0..1 (gsd_doubles) or
/// integer in 0..100 — same acceptance as the legacy service.
fn normalize_level(v: glib::Variant) -> Option<f64> {
    if let Some(d) = v.get::<f64>() {
        return Some(d);
    }
    if let Some(i) = v.get::<i32>() {
        return Some(i.clamp(0, 100) as f64 / 100.0);
    }
    v.get::<u32>().map(|u| u.min(100) as f64 / 100.0)
}
