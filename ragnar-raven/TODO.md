# ragnar-raven TODO

## Design rules

- Two roles, one process: windowless daemon (reminder loop +
  org.freedesktop.Notifications server) and `--day=YYYYMMDD` day
  window; later invocations are D-Bus-forwarded to the running one
- Data lives in standard formats other components read directly:
  appointments as iCalendar (`~/.local/share/ragnar-raven/
  appointments.ics`), notification history as JSONL
  (`history.jsonl`, max 200) — no private D-Bus API needed for
  the taskbar to see them
- Card windows use `KeyboardMode::None` — a notification must never
  steal input focus
- Store writes are atomic (temp + rename) — a crash must never
  leave a half-written .ics or history behind

## Open

- [ ] Notification area in the day window is a static snapshot of 5
      entries — should update live and allow dismiss/clear

## Later — cloud track (explicitly deprioritised)

- [ ] Full RFC 5545 round-trip: RRULE (recurring), DTEND/duration,
      LOCATION and preservation of unknown VEVENT properties on
      save — needed before real CalDAV import (DESCRIPTION is
      already mapped since 0.1.7)
- [ ] CalDAV sync (Nextcloud/ownCloud) — .ics format already
      compatible
- [ ] Central cloud credentials via the freedesktop Secret Service
      (`org.freedesktop.secrets`, e.g. gnome-keyring) — Ragnar
      Settings writes them, Raven/Valkyrie/etc. read them at sync
      time

## Done (recent)

- [x] Taskbar integration: calendar markers, right-click menu (add
      appointment, notifications on/off), double-click opens Raven
      at that day (`--day` via GApplication/D-Bus)
- [x] Reminder daemon: VALARM times, right-edge slide-in,
      auto-dismiss
- [x] Slide-in is the generic notification component (icon/title/
      body/duration/multi-monitor targeting) — notification server
      and appointment reminders share it
- [x] Display duration (`duration_s`) + optional sound (`sound`,
      freedesktop event id via libcanberra in-process)
      from settings.ini
- [x] Notification server (org.freedesktop.Notifications) —
      replaces_id, CloseNotification and NotificationClosed are
      spec-correct since 0.1.1 (card registry per id)
- [x] Persisted history (`history.jsonl`, atomic writes since 0.1.1)
- [x] History retention (0.1.4): `history_days` prunes aged entries
      on load, on append and at display time — the taskbar bell sees
      the same pruned file
- [x] Autostart entry (`etc/xdg/autostart/ragnar-raven.desktop`)
- [x] Translations (gettext domain `ragnar-raven`, .mo compiled into
      the .deb), .desktop file, .deb packaging
- [x] Replaced ragnar-daemon's Raven — the daemon no longer exists
      in the tree
- [x] Single day window (0.1.3): repeated --day activations retarget
      the open window instead of stacking new ones at the edge
- [x] Corrupt settings repair (0.1.5): settings.ini is read with a
      lenient line parser (GKeyFile rejects the whole file on one
      malformed line) and sanitize_settings() merges broken
      `[section` headers + duplicate sections on startup — heals
      files written by the buggy ragnar-settings<1.0.10 writer
- [x] ragnar-settings UI complete (1.0.11): every `[notifications]`
      key is exposed — enabled, duration_s, monitor, history_days
      (Notifications panel), watch_mounts/watch_updates/
      update_interval_min (Events card), color_* (Colours card) and
      sound/sound_volume (System Sounds panel)
- [x] Event watchers (0.1.6): src/watch.rs turns producerless events
      into notifications — VolumeMonitor (drive connect/disconnect,
      mount/unmount) and a periodic `apt-get -s` upgrade check;
      live-toggled via FileMonitor on settings.ini (watch_mounts,
      watch_updates, update_interval_min)
- [x] Category accent colours (0.1.6): color_default/color_mount/
      color_update/color_appointment paint the card's left edge;
      local producers (watch.rs, reminders) pass their category,
      bus notifications render as Default (no error concept in
      Raven — system errors are a separate tool's job later)
- [x] Appointment editing (0.1.7): pencil button per row opens the
      same editor pre-filled; save goes through Store::update and
      keeps the UID. FIRED dedup keys on (uid, offset, start) so a
      moved appointment re-arms its reminders
- [x] Appointment note + per-appointment sound (0.1.7): the editor
      gained a DESCRIPTION field (shown in the day list and the
      reminder card body) and a file picker for a custom alert
      sound — the picked file is copied into
      ~/.local/share/ragnar-raven/sounds/ and persisted as
      X-RAGNAR-SOUND in the VEVENT; the reminder plays it via
      libcanberra media.filename, falling back to the global sound.
      Editor content scrolls, buttons stay fixed
- [x] Module split (0.1.7): card.rs (slide-in surface, registry,
      accents, sounds) and ical.rs (datetime/VALARM helpers)
      extracted from reminder.rs and store.rs — every file back
      under the 400-line budget
