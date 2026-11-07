// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-automount — session automounter for the Ragnar desktop
// Copyright (C) 2026 LieraOS Project
//
// Mounts every mountable volume that is not mounted yet — once at
// session start (fixed internal disks, the live boot medium, media
// already attached) and again whenever the volume monitor announces
// a hot-plugged device. This is a *system* responsibility, not the
// file manager's: Valkyrie only lists devices, mounting happens here.
//
// The kill switch toggled by the "Removable media" panel in Ragnar
// Settings is the GSettings key org.lieraos.ragnar.automount enabled
// (schema shipped in data/, installed by the ragnar-settings package).
// When the schema or key is absent the daemon defaults to enabled, so
// shipping this binary alone already yields the desired behaviour.
//
// Errors are deliberately silent: a volume that refuses to mount
// stays visible in the file manager sidebar where the user can mount
// it with one click — an error dialog on every login would be noise.

use gio::prelude::*;
use glib::MainLoop;

/// The GSettings key checked before every mount — read per event so
/// the Settings toggle applies live without restarting the session.
const SCHEMA: &str = "org.lieraos.ragnar.automount";
const KEY: &str = "enabled";

/// Whether automounting is enabled right now. Defaults to true when
/// the schema or key does not exist on this system.
fn automount_enabled() -> bool {
    let Some(source) = gio::SettingsSchemaSource::default() else {
        return true;
    };
    let Some(schema) = source.lookup(SCHEMA, true) else {
        return true;
    };
    if !schema.list_keys().iter().any(|k| k == KEY) {
        return true;
    }
    gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None).boolean(KEY)
}

/// Mount `volume` when it is mountable and has no mount yet.
/// `volume-added` also fires for internal plumbing (loop devices,
/// GVFS shadows) — those report can_mount() = false or already carry
/// a mount, so they are skipped naturally.
fn mount_if_needed(volume: &gio::Volume) {
    if !volume.can_mount() || volume.get_mount().is_some() {
        return;
    }
    let volume = volume.clone();
    glib::MainContext::default().spawn_local(async move {
        // No parent window: automount must not block on a dialog
        // position; GVFS picks the active window itself.
        let _ = volume
            .mount_future(
                gio::MountMountFlags::NONE,
                Some(&gio::MountOperation::new()),
            )
            .await;
    });
}

/// Mount all volumes when automount is enabled.
fn mount_all(monitor: &gio::VolumeMonitor) {
    if !automount_enabled() {
        return;
    }
    for volume in monitor.volumes() {
        mount_if_needed(&volume);
    }
}

fn main() {
    let monitor = gio::VolumeMonitor::get();

    // Mount everything already present at session start.
    mount_all(&monitor);

    // And every volume plugged in later — the enable check runs per
    // event so disabling the feature in Settings takes effect live.
    monitor.connect_volume_added(move |_, volume| {
        // Real hot-plug event: the configured event sound fires even
        // when automounting is disabled — they are separate settings.
        let _ = std::process::Command::new("lieraaudio-sound")
            .arg("device-added")
            .spawn();
        if automount_enabled() {
            mount_if_needed(volume);
        }
    });
    monitor.connect_volume_removed(move |_, _| {
        let _ = std::process::Command::new("lieraaudio-sound")
            .arg("device-removed")
            .spawn();
    });

    eprintln!("ragnar-automount: running");
    MainLoop::new(None, false).run();
}
