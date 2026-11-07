// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-keyboard-layout — the D-Bus surface
//
// Owns org.lieraos.ragnar.KeyboardLayout at
// /org/lieraos/ragnar/KeyboardLayout. Thin broker, identical contract
// to the legacy KeyboardLayoutManager it replaces:
//
//   property CurrentLayout (rw, "s") — read by the panel applet,
//       written by the bridge after a layout change took effect;
//       changes are re-emitted as PropertiesChanged.
//   method  SetKeyboardLayout(s)     — request a specific layout;
//       re-emitted as the LayoutChanged signal.
//   methods SwitchLayoutNext/Previous — rotation requests from
//       compositor keybinds; re-emitted as the matching
//       SwitchLayout*Requested signals.
//
// All heavy lifting (writing XKB_DEFAULT_LAYOUT, reloading the
// compositor, locale1 sync) happens in the bridge — this service
// deliberately stays a pure broker.

use crate::state;
use gio::prelude::*;

const NAME: &str = "org.lieraos.ragnar.KeyboardLayout";
const PATH: &str = "/org/lieraos/ragnar/KeyboardLayout";

const XML: &str = r#"<node>
  <interface name="org.lieraos.ragnar.KeyboardLayout">
    <property name="CurrentLayout" type="s" access="readwrite"/>
    <method name="SetKeyboardLayout"><arg type="s" direction="in"/></method>
    <method name="SwitchLayoutNext"/>
    <method name="SwitchLayoutPrevious"/>
    <signal name="LayoutChanged"><arg type="s"/></signal>
    <signal name="SwitchLayoutNextRequested"/>
    <signal name="SwitchLayoutPreviousRequested"/>
  </interface>
</node>"#;

/// Owns the bus name; the OwnerId must be kept alive for the
/// process lifetime.
pub fn own_name() -> gio::OwnerId {
    gio::bus_own_name(
        gio::BusType::Session,
        NAME,
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| register(&conn),
        |_, _| {},
        |_, _| eprintln!("ragnar-keyboard-layout: lost bus name {NAME}"),
    )
}

/// Registers the broker object and wires up handlers.
fn register(conn: &gio::DBusConnection) {
    let node = gio::DBusNodeInfo::for_xml(XML).expect("valid xml");
    let Some(info) = node.lookup_interface(NAME) else {
        return;
    };
    conn.register_object(PATH, &info)
        .method_call(|conn, _, _, _, method, params, inv| {
            match method {
                // Forward a concrete layout request to the bridge.
                "SetKeyboardLayout" => {
                    if let Some(layout) = params.child_value(0).get::<String>() {
                        emit(&conn, "LayoutChanged", Some(&layout.to_variant()));
                    }
                }
                // Rotation requests from compositor keybinds.
                "SwitchLayoutNext" => emit(&conn, "SwitchLayoutNextRequested", None),
                "SwitchLayoutPrevious" => emit(&conn, "SwitchLayoutPreviousRequested", None),
                _ => {}
            }
            inv.return_value(None);
        })
        .property(|_, _, _, _, prop| match prop {
            "CurrentLayout" => state::get().to_variant(),
            _ => String::new().to_variant(),
        })
        .set_property(|conn, _, _, _, prop, value| {
            // Bridge writes the applied layout back; notify subscribers
            // (applet) through the standard PropertiesChanged signal.
            if prop != "CurrentLayout" {
                return false;
            }
            let Some(layout) = value.get::<String>() else {
                return false;
            };
            if state::set(&layout) {
                let changed = {
                    let b = glib::VariantDict::new(None);
                    b.insert_value("CurrentLayout", &layout.to_variant());
                    b.end()
                };
                let _ = conn.emit_signal(
                    None,
                    PATH,
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    Some(&glib::Variant::tuple_from_iter([
                        NAME.to_variant(),
                        changed,
                        Vec::<String>::new().to_variant(),
                    ])),
                );
            }
            true
        })
        .build();
}

/// Emits one of the broker's own signals.
fn emit(conn: &gio::DBusConnection, signal: &str, arg: Option<&glib::Variant>) {
    let payload = match arg {
        Some(v) => glib::Variant::tuple_from_iter([v.clone()]),
        None => glib::Variant::tuple_from_iter(std::iter::empty::<glib::Variant>()),
    };
    let _ = conn.emit_signal(None, PATH, NAME, signal, Some(&payload));
}
