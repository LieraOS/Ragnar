// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-wallpaper — AccountsService background sync
//
// Mirrors the user's wallpaper into AccountsService so the display
// manager greeter shows the same background — the same dual call
// the legacy daemon made (standard User.SetBackgroundFile plus the
// DisplayManager extension property, LightDM convention).

use gio::prelude::*;

const NAME: &str = "org.freedesktop.Accounts";
const OBJ: &str = "/org/freedesktop/Accounts";
const USER_IFACE: &str = "org.freedesktop.Accounts.User";
const DM_PROP: &str = "org.freedesktop.DisplayManager.AccountsService";

/// Sets the greeter background file for the calling user; failures
/// are logged but never fatal — the greeter sync is best-effort.
pub fn set_background(path: &str) {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) else {
        return;
    };
    let Ok(user) = conn.call_sync(
        Some(NAME),
        OBJ,
        NAME,
        "FindUserByName",
        Some(&glib::Variant::tuple_from_iter([
            glib::user_name().to_variant()
        ])),
        Some(glib::VariantTy::new("(o)").unwrap()),
        gio::DBusCallFlags::NONE,
        10_000,
        gio::Cancellable::NONE,
    ) else {
        return;
    };
    let Some(upath) = user
        .child_value(0)
        .get::<String>()
        .filter(|s| !s.is_empty())
    else {
        return;
    };

    // Standard interface first.
    let _ = conn.call_sync(
        Some(NAME),
        &upath,
        USER_IFACE,
        "SetBackgroundFile",
        Some(&glib::Variant::tuple_from_iter([path.to_variant()])),
        None,
        gio::DBusCallFlags::NONE,
        10_000,
        gio::Cancellable::NONE,
    );

    // DisplayManager extension property (LightDM convention).
    let _ = conn.call_sync(
        Some(NAME),
        &upath,
        "org.freedesktop.DBus.Properties",
        "Set",
        Some(&glib::Variant::tuple_from_iter([
            DM_PROP.to_variant(),
            "BackgroundFile".to_variant(),
            path.to_variant().as_variant().to_variant(),
        ])),
        None,
        gio::DBusCallFlags::NONE,
        10_000,
        gio::Cancellable::NONE,
    );
}
