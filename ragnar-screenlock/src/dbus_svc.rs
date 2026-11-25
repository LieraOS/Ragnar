// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-screenlock — the D-Bus service surface
//
// Owns org.lieraos.ragnar.Screenlock and registers
// /org/lieraos/ragnar/Screenlock with the same methods the legacy
// daemon exposed — keybinds (Super+L), swayidle hooks and the
// suspend path keep calling them unchanged.

use crate::{backlight, battery, locker, settings};
use gio::prelude::*;

const NAME: &str = "org.lieraos.ragnar.Screenlock";
const PATH: &str = "/org/lieraos/ragnar/Screenlock";

/// Introspection XML for the exported interface.
const XML: &str = r#"<node>
  <interface name="org.lieraos.ragnar.Screenlock">
    <method name="Lock"/>
    <method name="Dim"/>
    <method name="Undim"/>
    <method name="ScreenPowerSuspend"/>
    <method name="ScreenPowerResume"/>
    <method name="OnBattery"><arg type="b" direction="out"/></method>
  </interface>
</node>"#;

/// Owns the bus name (REPLACE so we can take it from the legacy
/// daemon during the transition) and registers the object once the
/// name is ours. Returns the OwnerId — keep it alive for the
/// process lifetime.
pub fn own_name() -> gio::OwnerId {
    gio::bus_own_name(
        gio::BusType::Session,
        NAME,
        gio::BusNameOwnerFlags::REPLACE,
        |conn, _| register(&conn),
        |_, _| {},
        |_, _| eprintln!("ragnar-screenlock: lost bus name {NAME}"),
    )
}

/// Registers the Screenlock object on the session connection and
/// handles method dispatch.
fn register(conn: &gio::DBusConnection) {
    let node = gio::DBusNodeInfo::for_xml(XML).expect("valid xml");
    let Some(info) = node.lookup_interface(NAME) else {
        return;
    };
    let _ = conn.register_object(PATH, &info)
        .method_call(|_c, _s, _p, _i, method, _params, inv| {
            match method {
                "Lock" => spawn_lock(),
                "Dim" => backlight::dim(settings::snapshot().idle_brightness),
                "Undim" => backlight::undim(),
                "ScreenPowerSuspend" | "ScreenPowerResume" => {}
                "OnBattery" => {
                    inv.return_value(Some(&battery::on_battery().to_variant()));
                    return;
                }
                _ => {}
            }
            inv.return_value(None);
        })
        .build();
}

/// Spawns the locker command built from the current settings.
fn spawn_lock() {
    let cmd = locker::command(&settings::snapshot().picture_uri);
    if cmd.is_empty() {
        return;
    }
    let _ = gio::AppInfo::create_from_commandline(&cmd, None, gio::AppInfoCreateFlags::NONE)
        .and_then(|a| a.launch(&[], gio::AppLaunchContext::NONE));
}
