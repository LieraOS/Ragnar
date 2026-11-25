// Copyright (C) Jörg Schröder
//! Event watchers — translate hardware and package events into
//! Raven notifications.
//!
//! Raven is the desktop's notification hub: it already owns
//! org.freedesktop.Notifications and fires appointment reminders.
//! The events here have no producer of their own — nobody sends a
//! bus message when a USB drive appears — so Raven watches the
//! sources directly and turns them into ordinary notifications
//! (history entry + slide-in card + category colour).
//!
//! Every watcher is toggleable through settings.ini live: a
//! FileMonitor on the ini re-evaluates the source switches on every
//! write, so ragnar-settings changes apply without a restart.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::card::Category;
use crate::i18n::{tr, trn};

/// Minutes between update checks when `update_interval_min` is
/// absent — hourly matches how fast apt indexes refresh anyway.
const UPDATE_INTERVAL_DEFAULT_MIN: u32 = 60;

thread_local! {
    /// VolumeMonitor signal handlers — non-empty while the drive
    /// watcher is active; disconnecting turns it off without
    /// restarting the daemon.
    static MOUNT_SIGS: std::cell::RefCell<Vec<glib::SignalHandlerId>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// The VolumeMonitor itself — the reference must outlive
    /// set_mounts(): dropping the last handle tears down the union
    /// monitor's remote proxies, so D-Bus events still reach the
    /// process but no mount/drive signal is ever emitted again.
    static VOL_MON: std::cell::RefCell<Option<gio::VolumeMonitor>> =
        const { std::cell::RefCell::new(None) };
    /// GLib source of the update-check timer (None = watcher off).
    static UPDATE_SRC: std::cell::RefCell<Option<glib::SourceId>> =
        const { std::cell::RefCell::new(None) };
    /// Interval the running update timer was armed with — refresh()
    /// rebuilds the timer when the configured value changes.
    static UPDATE_ARMED_MIN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Pending-package count that last triggered a card — the same
    /// count twice must not notify twice.
    static LAST_UPDATE_COUNT: std::cell::Cell<i64> = const { std::cell::Cell::new(-1) };
    /// settings.ini monitor — kept alive for the process lifetime so
    /// toggles and interval changes apply immediately.
    static CFG_MON: std::cell::RefCell<Option<gio::FileMonitor>> =
        const { std::cell::RefCell::new(None) };
}

/// Start the watcher subsystem: arm the ini monitor, then enable
/// every source the settings allow. Idempotent — forwarded --day
/// activations must not stack monitors or timers.
pub fn start() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if ONCE.is_completed() {
        return;
    }
    ONCE.call_once(|| ());
    // Restore the persisted update baseline: without it every
    // restart compares the first successful check against "unknown"
    // and re-notifies about the same unchanged updates.
    LAST_UPDATE_COUNT.with(|c| c.set(load_update_count()));
    // Watch the config DIRECTORY, not the file: our writers replace
    // settings.ini via temp+rename, which swaps the inode and would
    // kill a file-level monitor after the first write. The dir
    // monitor also catches the file appearing for the first time.
    if let Some(dir) = crate::util::settings_path().parent() {
        let dmon = gio::File::for_path(dir)
            .monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE);
        if let Ok(mon) = dmon {
            mon.connect_changed(|_, f, _, ev| {
                let is_ini = f
                    .basename()
                    .map(|b| b.as_os_str() == "settings.ini")
                    .unwrap_or(false);
                if is_ini
                    && matches!(
                        ev,
                        gio::FileMonitorEvent::Changed
                            | gio::FileMonitorEvent::Created
                            | gio::FileMonitorEvent::Renamed
                            | gio::FileMonitorEvent::MovedIn
                    )
                {
                    refresh();
                }
            });
            CFG_MON.with(|m| *m.borrow_mut() = Some(mon));
        }
    }
    refresh();
}

/// Re-evaluate every source switch against the current ini. Called
/// once at startup and on each settings.ini change.
fn refresh() {
    set_mounts(crate::util::ini_bool("notifications", "watch_mounts").unwrap_or(true));
    set_updates(crate::util::ini_bool("notifications", "watch_updates").unwrap_or(true));
}

// ------------------------------------------------------------------ //
// Removable media — GIO's VolumeMonitor emits for USB sticks,       //
// cards, phones and network mounts alike.                           //
// ------------------------------------------------------------------ //

/// Connect or drop the VolumeMonitor signals depending on the
/// configured switch — a no-op when the state already matches.
fn set_mounts(want: bool) {
    let have = MOUNT_SIGS.with(|s| !s.borrow().is_empty());
    if want == have {
        return;
    }
    let vm = gio::VolumeMonitor::get();
    if want {
        let ids = vec![
            vm.connect_mount_added(|_, m| announce_mount(m, true)),
            vm.connect_mount_removed(|_, m| announce_mount(m, false)),
            vm.connect_drive_connected(|_, d| announce_drive(d, true)),
            vm.connect_drive_disconnected(|_, d| announce_drive(d, false)),
        ];
        MOUNT_SIGS.with(|s| *s.borrow_mut() = ids);
        // See VOL_MON above — releasing this reference would take
        // the freshly connected handlers down with the monitor.
        VOL_MON.with(|v| *v.borrow_mut() = Some(vm));
    } else {
        let ids = MOUNT_SIGS.with(|s| s.take());
        for id in ids {
            vm.disconnect(id);
        }
    }
}

/// Card for a (un)mount — e.g. the user mounting a partition.
fn announce_mount(m: &gio::Mount, added: bool) {
    crate::notify::announce_local(
        Category::Mount,
        Some(&m.icon()),
        &tr(if added {
            "Laufwerk eingehängt"
        } else {
            "Laufwerk ausgehängt"
        }),
        &m.name(),
    );
}

/// Card for a drive (dis)connect — the physical USB plug event.
fn announce_drive(d: &gio::Drive, added: bool) {
    crate::notify::announce_local(
        Category::Mount,
        Some(&d.icon()),
        &tr(if added {
            "Laufwerk angeschlossen"
        } else {
            "Laufwerk getrennt"
        }),
        &d.name(),
    );
}

// ------------------------------------------------------------------ //
// Pending package upgrades — the same non-destructive simulation   //
// the settings Updates panel uses: `apt-get -s` needs neither      //
// root nor the apt lock, and "Inst" lines are locale-proof.        //
// ------------------------------------------------------------------ //

/// Arm or drop the update-check timer. Rebuilds when the configured
/// interval changed so settings apply on the next refresh.
fn set_updates(want: bool) {
    let min = crate::util::ini_int("notifications", "update_interval_min")
        .map(|m| m.clamp(5, 1440) as u32)
        .unwrap_or(UPDATE_INTERVAL_DEFAULT_MIN);
    let armed = UPDATE_SRC.with(|s| s.borrow().is_some());
    let interval_changed = UPDATE_ARMED_MIN.with(|m| m.get() != min);
    if want && armed && !interval_changed {
        return;
    }
    // Rebuild: drop the old timer (if any) first.
    if let Some(src) = UPDATE_SRC.with(|s| s.take()) {
        src.remove();
    }
    if !want {
        UPDATE_ARMED_MIN.with(|m| m.set(0));
        return;
    }
    UPDATE_ARMED_MIN.with(|m| m.set(min));
    // First check shortly after start (let the session settle),
    // then on the configured interval.
    glib::timeout_add_seconds_local_once(30, check_updates);
    let src = glib::timeout_add_seconds_local(min * 60, || {
        check_updates();
        glib::ControlFlow::Continue
    });
    UPDATE_SRC.with(|s| *s.borrow_mut() = Some(src));
}

/// Run the apt simulation off the GTK loop and report on the main
/// context — apt can take a second or two and must not stall cards.
fn check_updates() {
    std::thread::spawn(|| {
        let count = std::process::Command::new("apt-get")
            .args(["-s", "-o", "Debug::NoLocking=1", "dist-upgrade"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| l.starts_with("Inst "))
                    .count()
            });
        // Non-_local idle_add: this runs on a worker thread where the
        // default main context is already owned by the GTK loop — the
        // _local variant tries to acquire it and panics, which silently
        // killed every update notification before this fix.
        glib::idle_add_once(move || report_updates(count));
    });
}

/// Path of the small state file remembering which pending-update
/// count was last reported — lives in the data dir, not the config
/// dir, so the settings FileMonitor ignores it.
fn update_count_path() -> std::path::PathBuf {
    glib::user_data_dir()
        .join("ragnar-raven")
        .join("update-count")
}

/// Read the persisted baseline; -1 when no state exists yet (fresh
/// install) — the first successful check then reports once.
fn load_update_count() -> i64 {
    std::fs::read_to_string(update_count_path())
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(-1)
}

/// Persist a newly reported count — best-effort, a failed write just
/// means the next start may re-notify once.
fn save_update_count(count: i64) {
    let p = update_count_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, count.to_string());
}

/// Notify only on a CHANGED pending count — 0→5 alerts, 5→5 stays
/// silent, 5→0 resets (the next >0 will alert again). The baseline
/// survives restarts via update-count, so relaunching the daemon
/// does not repeat a notification for an unchanged set. A failed
/// simulation keeps the previous state.
fn report_updates(count: Option<usize>) {
    let Some(count) = count else { return };
    let changed = LAST_UPDATE_COUNT.with(|l| {
        let prev = l.replace(count as i64);
        prev != count as i64
    });
    if changed {
        save_update_count(count as i64);
    }
    if count > 0 && changed {
        crate::notify::announce_local(
            Category::Update,
            Some(&gio::ThemedIcon::new("system-software-update").upcast()),
            &tr("Updates verfügbar"),
            &format!(
                "{} {}",
                count,
                trn("ausstehendes Paket", "ausstehende Pakete", count as u64)
            ),
        );
    }
}
