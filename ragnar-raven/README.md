# ragnar-raven

Ragnar's appointment + notification component in Rust + GTK4 —
appointment management, reminders, and the freedesktop notification
server `org.freedesktop.Notifications`: incoming notifications are
shown as a slide-in card and collected in a persisted history. One
process, two roles:

- **Daemon mode** (`ragnar-raven` with no arguments): runs
  windowlessly in the background, watches the VALARM times of all
  appointments and on a due reminder shows a small card that slides
  in from the right screen edge, stays ~18 s and slides back out.
  Kept alive via `app.hold()` — no window required. Intended as a
  session autostart.
- **Window mode** (`ragnar-raven --day=YYYYMMDD`): opens the compact
  day window at the top right (layer-shell, ~360 px wide, height
  follows content) with the day's appointment list and entry into
  the appointment editor. When the daemon is already running, the
  call is forwarded to it via GApplication/D-Bus — there is always
  exactly one process.

The slide-in card is a generic notification component (icon, title,
body): appointment reminders and `org.freedesktop.Notifications`
messages share the same path. Notification cards are tracked by id,
so `CloseNotification` and `replaces_id` close them for real and the
spec-mandated `NotificationClosed` signal is emitted. The card shows
on all monitors or a single one, depending on
`~/.config/ragnar-raven/settings.ini`, section `[notifications]`:
`monitor = all` (default, one card per monitor) or a connector name
(`DP-3`, `HDMI-A-1` …) to pin it; `enabled = true|false` is the
global on/off switch; `duration_s` sets hold time; `sound` an
optional freedesktop sound-event id (played via
libcanberra in-process); `history_days` limits how long past
notifications stay in the list (0 = keep nothing, -1/absent =
unbounded). The card's left-edge accent can be coloured per
category via `color_default`, `color_mount`,
`color_update` and `color_appointment` (any CSS colour). All keys
are set conveniently through `ragnar-settings`.

Event watchers: events that have no notification producer of their
own are watched by Raven itself (`src/watch.rs`) and turned into
ordinary notifications — removable media (drive connected/
disconnected, mount/unmount via GIO's VolumeMonitor) and pending
package upgrades (`apt-get -s dist-upgrade` simulation, no root, no
lock; alerts only when the count changes). `watch_mounts` and
`watch_updates` toggle them (default on), `update_interval_min`
sets the check interval (default 60). A monitor on the config
directory re-evaluates the switches live — no restart needed.

Appointments live as an iCalendar file at
`~/.local/share/ragnar-raven/appointments.ics` — one VEVENT per
appointment, one VALARM per reminder offset, `DESCRIPTION` for the
optional note and `X-RAGNAR-SOUND` for a per-appointment alert file
(the picked file is copied into
`~/.local/share/ragnar-raven/sounds/` so moving or deleting the
original never breaks the appointment; without one the global
notification sound applies). CalDAV-compatible for the planned
Nextcloud/ownCloud sync. Store writes are atomic so a crash never
leaves a half-written file.

Taskbar integration: marked days in the clock calendar, right-click
menu (add appointment / toggle notifications), double-click opens
Raven at the chosen day; the notification bell applet reads the
shared history.jsonl.

## Build

```bash
cargo build --release
```

## Layout

- `src/store.rs` — .ics read/write, appointment model, sound-copy
  lifecycle
- `src/ical.rs` — iCalendar helpers (datetime parse/format,
  VALARM triggers)
- `src/sidebar.rs` — compact day window (layer-shell top-right)
- `src/dialog.rs` — appointment editor (title, note, time, reminder
  presets + custom offset, per-appointment sound picker)
- `src/reminder.rs` — reminder daemon loop (VALARM scheduling)
- `src/card.rs` — the slide-in card: category accents, per-monitor
  windows, card registry, card sounds
- `src/notify.rs` — org.freedesktop.Notifications server + history
- `src/main.rs` — application entry, single-instance/D-Bus wiring
- `src/util.rs` — atomic file writes
- `src/sound.rs` — libcanberra FFI for event sounds
- `src/watch.rs` — VolumeMonitor + apt watchers feeding the same notification path
- `src/i18n.rs` — gettext wrapper (`ragnar-raven` text domain)
