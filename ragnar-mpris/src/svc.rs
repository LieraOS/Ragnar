// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-mpris — the D-Bus surface
//
// Owns org.lieraos.ragnar.MprisController at
// /org/lieraos/ragnar/MprisController and forwards media-key
// methods to the current MPRIS player's standard interface —
// identical names and semantics to the legacy service, so existing
// keybinds keep working untouched.

use crate::tracker;
use gio::prelude::*;
use glib::VariantTy;

const NAME: &str = "org.lieraos.ragnar.MprisController";
const PATH: &str = "/org/lieraos/ragnar/MprisController";

/// Methods the fork exposed — forwarded to the player verbatim.
const FORWARDED: &[&str] = &["PlayPause", "Play", "Pause", "Next", "Previous", "Stop"];

const XML: &str = r#"<node>
  <interface name="org.lieraos.ragnar.MprisController">
    <method name="CurrentPlayer"><arg type="s" direction="out"/></method>
    <method name="PlayPause"/><method name="Play"/><method name="Pause"/>
    <method name="Next"/><method name="Previous"/><method name="Stop"/>
  </interface>
</node>"#;

/// Owns the bus name and registers the object; returns the OwnerId
/// to hold for the process lifetime.
pub fn own_name() -> gio::OwnerId {
    gio::bus_own_name(
        gio::BusType::Session,
        NAME,
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| register(&conn),
        |_, _| {},
        |_, _| eprintln!("ragnar-mpris: lost bus name {NAME}"),
    )
}

/// Registers the controller object and dispatches methods.
fn register(conn: &gio::DBusConnection) {
    let node = gio::DBusNodeInfo::for_xml(XML).expect("valid xml");
    let Some(info) = node.lookup_interface(NAME) else {
        return;
    };
    let conn2 = conn.clone();
    conn.register_object(PATH, &info)
        .method_call(move |_, _, _, _, method, _params, inv| {
            if method == "CurrentPlayer" {
                let id = player_identity();
                inv.return_value(Some(&glib::Variant::tuple_from_iter([id.to_variant()])));
                return;
            }
            if FORWARDED.contains(&method) {
                if let Some(player) = tracker::current() {
                    let _ = conn2.call_sync(
                        Some(&player),
                        "/org/mpris/MediaPlayer2",
                        "org.mpris.MediaPlayer2.Player",
                        method,
                        None,
                        None::<&VariantTy>,
                        gio::DBusCallFlags::NONE,
                        5_000,
                        gio::Cancellable::NONE,
                    );
                }
            }
            inv.return_value(None);
        })
        .build();
}

/// Identity of the current player (Identity property), empty when
/// no player is around — same contract as the legacy method.
fn player_identity() -> String {
    let Some(player) = tracker::current() else {
        return String::new();
    };
    let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return String::new();
    };
    conn.call_sync(
        Some(&player),
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
        "Get",
        Some(&glib::Variant::tuple_from_iter([
            "org.mpris.MediaPlayer2".to_variant(),
            "Identity".to_variant(),
        ])),
        None,
        gio::DBusCallFlags::NONE,
        5_000,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|v| v.child_value(0).get::<glib::Variant>())
    .and_then(|v| v.get::<String>())
    .unwrap_or_default()
}
