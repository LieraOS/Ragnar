# ragnar-raven

**Deutsch:** Neufassung von Raven in Rust + GTK4 — Terminverwaltung, Erinnerungen und Benachrichtigungen. Hostet außerdem den freedesktop-Notification-Server `org.freedesktop.Notifications` (übernimmt den Namen per REPLACE von `ragnar-daemon`): eingehende Benachrichtigungen werden als Slide-in-Karte angezeigt und im Verlauf gesammelt. Das Programm hat zwei Rollen in einem Prozess:

- **Daemon-Modus** (`ragnar-raven` ohne Argumente): läuft fensterlos im Hintergrund, überwacht die VALARM-Zeitpunkte aller Termine und zeigt bei Fälligkeit eine kleine Benachrichtigungskarte, die von der rechten Bildschirmkante hereinslidet, ~18 s bleibt und wieder verschwindet. Anwendung über `app.hold()` — kein Fenster nötig. Gedacht als Session-Autostart.
- **Fenster-Modus** (`ragnar-raven --day=YYYYMMDD`): öffnet das kompakte Tagesfenster oben rechts (Layer-Shell, ~360 px breit, Höhe folgt dem Inhalt) mit Tages-Terminliste und Einstieg in den Termin-Editor. Bei laufendem Daemon wird der Aufruf per GApplication/D-Bus an die bestehende Instanz weitergeleitet — es gibt immer nur einen Prozess.

Die Slide-in-Karte ist eine generische Benachrichtigungskomponente (Icon, Titel, Text): Termin-Erinnerungen, `org.freedesktop.Notifications`-Nachrichten und später weitere Systemmeldungen nutzen denselben Pfad. Sie erscheint je Konfiguration auf allen Monitoren oder nur auf einem — `~/.config/ragnar-raven/settings.ini`, Sektion `[notifications]`: `monitor = all` (Standard, eine Karte je Monitor) oder Connector-Name (`DP-3`, `HDMI-A-1` …) zum Pinnen; `enabled = true|false` ist der globale Ein/Aus-Schalter. Beide Keys werden später über `ragnar-settings` komfortabel gesetzt.

Termine liegen als iCalendar-Datei unter `~/.local/share/ragnar-raven/appointments.ics` — ein VEVENT pro Termin, ein VALARM pro Erinnerungszeitpunkt. CalDAV-kompatibel für die geplante Nextcloud/ownCloud-Synchronisation. Der globale Benachrichtigungs-Schalter wird u. a. über das Rechtsklick-Menü des Taskbar-Kalenders bedient.

Die Taskbar-Anbindung: markierte Tage im Uhren-Kalender, Rechtsklick-Menü (Termin eintragen / Benachrichtigungen schalten), Doppelklick öffnet Raven am gewählten Tag. Ersetzt schrittweise das Raven aus `ragnar-daemon`.

**English:** Reimplementation of Raven in Rust + GTK4 — appointment management, reminders and notifications. It also hosts the freedesktop notification server `org.freedesktop.Notifications` (takes the name over from `ragnar-daemon` via REPLACE): incoming notifications are shown as a slide-in card and collected in the history. One process, two roles:

- **Daemon mode** (`ragnar-raven` with no arguments): runs windowlessly in the background, watches the VALARM times of all appointments and on a due reminder shows a small card that slides in from the right screen edge, stays ~18 s and slides back out. Kept alive via `app.hold()` — no window required. Intended as a session autostart.
- **Window mode** (`ragnar-raven --day=YYYYMMDD`): opens the compact day window at the top right (layer-shell, ~360 px wide, height follows content) with the day's appointment list and entry into the appointment editor. When the daemon is already running, the call is forwarded to it via GApplication/D-Bus — there is always exactly one process.

The slide-in card is a generic notification component (icon, title, body): appointment reminders, `org.freedesktop.Notifications` messages and later other system messages all use the same path. It shows on all monitors or a single one, depending on `~/.config/ragnar-raven/settings.ini`, section `[notifications]`: `monitor = all` (default, one card per monitor) or a connector name (`DP-3`, `HDMI-A-1` …) to pin it; `enabled = true|false` is the global on/off switch. Both keys will later be set conveniently through `ragnar-settings`.

Appointments live as an iCalendar file at `~/.local/share/ragnar-raven/appointments.ics` — one VEVENT per appointment, one VALARM per reminder offset. CalDAV-compatible for the planned Nextcloud/ownCloud sync. The global notification switch is toggled e.g. via the taskbar calendar's right-click menu.

Taskbar integration: marked days in the clock calendar, right-click menu (add appointment / toggle notifications), double-click opens Raven at the chosen day. 

## Build

```bash
cargo build --release
```

## Layout

- `src/store.rs` — .ics read/write, appointment model, reminder offsets
- `src/sidebar.rs` — compact day window (layer-shell top-right)
- `src/dialog.rs` — appointment editor (title, time, reminder presets + custom offset)
- `src/reminder.rs` — reminder daemon loop + right-edge slide-in card
- `src/main.rs` — application entry, single-instance/D-Bus wiring
- `src/i18n.rs` — gettext wrapper (`ragnar-raven` text domain)
