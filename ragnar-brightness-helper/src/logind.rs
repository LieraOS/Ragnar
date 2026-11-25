// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-brightness-helper — the logind SetBrightness call
//
// logind lets the session owner change its own backlight without
// privileges: org.freedesktop.login1.Session.SetBrightness on the
// caller's own session object (/org/freedesktop/login1/session/self
// resolves to the session of the *calling* process — exactly what a
// keybind-triggered helper needs).

use gio::prelude::*;
use glib::{Variant, VariantTy};

/// Set `dev` (backlight subsystem name like "acpi_video0") to
/// absolute `value` via logind on the system bus.
pub fn set_brightness(dev: &str, value: u32) -> Result<(), glib::Error> {
    let conn = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE)?;
    conn.call_sync(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1/session/self",
        "org.freedesktop.login1.Session",
        "SetBrightness",
        Some(&Variant::tuple_from_iter([
            "backlight".to_variant(),
            dev.to_variant(),
            value.to_variant(),
        ])),
        Some(VariantTy::UNIT),
        gio::DBusCallFlags::NONE,
        10_000,
        gio::Cancellable::NONE,
    )?;
    Ok(())
}
