# ragnar-raven

Ragnars Termin- und Benachrichtigungs-Komponente in Rust + GTK4 —
Terminverwaltung, Erinnerungen und der freedesktop-
Benachrichtigungs-Server `org.freedesktop.Notifications`:
eingehende Benachrichtigungen erscheinen als Slide-in-Karte und
werden im Verlauf gesammelt. Ein Prozess, zwei Rollen:

- **Daemon-Modus** (`ragnar-raven` ohne Argumente): läuft fensterlos
  im Hintergrund, überwacht die VALARM-Zeitpunkte aller Termine und
  zeigt bei Fälligkeit eine kleine Karte, die von der rechten
  Bildschirmkante hereinslidet, ~18 s bleibt und wieder
  verschwindet. Anwendung über `app.hold()` — kein Fenster nötig.
  Gedacht als Session-Autostart.
- **Fenster-Modus** (`ragnar-raven --day=YYYYMMDD`): öffnet das
  kompakte Tagesfenster oben rechts (Layer-Shell, ~360 px breit,
  Höhe folgt dem Inhalt) mit Tages-Terminliste und Einstieg in den
  Termin-Editor. Bei laufendem Daemon wird der Aufruf per
  GApplication/D-Bus an die bestehende Instanz weitergeleitet — es
  gibt immer nur einen Prozess.

Die Slide-in-Karte ist eine generische Benachrichtigungskomponente
(Icon, Titel, Text): Termin-Erinnerungen und
`org.freedesktop.Notifications`-Nachrichten nutzen denselben Pfad.
Karten werden unter ihrer Notification-ID geführt —
`CloseNotification` und `replaces_id` schließen sie wirklich, und
das spec-pflichtige `NotificationClosed`-Signal wird gesendet. Die
Karte erscheint je Konfiguration auf allen Monitoren oder nur auf
einem — `~/.config/ragnar-raven/settings.ini`, Sektion
`[notifications]`: `monitor = all` (Standard, eine Karte je
Monitor) oder Connector-Name (`DP-3`, `HDMI-A-1` …) zum Pinnen;
`enabled = true|false` ist der globale Ein/Aus-Schalter;
`duration_s` die Anzeigedauer; `sound` eine optionale freedesktop-
Sound-Event-ID; `history_days` begrenzt, wie lange vergangene
Benachrichtigungen in der Liste bleiben (0 = nichts behalten,
-1/fehlend = unbegrenzt) (in-process über libcanberra). Die
Akzentfarbe am linken Kartenrand lässt sich pro Kategorie setzen:
`color_default`, `color_mount`, `color_update` und
`color_appointment` (beliebige CSS-Farbe). Alle Keys werden
komfortabel über `ragnar-settings` gesetzt.

Ereignis-Quellen: Vorfälle ohne eigenen Benachrichtigungs-
Produzenten beobachtet Raven selbst (`src/watch.rs`) und macht
daraus normale Benachrichtigungen — Wechselmedien (Laufwerk
angeschlossen/getrennt, eingehängt/ausgehängt via GIOs
VolumeMonitor) und ausstehende Paket-Upgrades (`apt-get -s
dist-upgrade`-Simulation, kein Root, kein Lock; meldet nur bei
geänderter Anzahl). `watch_mounts` und `watch_updates` schalten
sie (Standard: an), `update_interval_min` bestimmt den
Prüfintervall (Standard 60). Ein Monitor auf dem Konfigurations-
verzeichnis wertet die Schalter live aus — kein Neustart nötig.

Termine liegen als iCalendar-Datei unter
`~/.local/share/ragnar-raven/appointments.ics` — ein VEVENT pro
Termin, ein VALARM pro Erinnerungszeitpunkt, `DESCRIPTION` für die
optionale Notiz und `X-RAGNAR-SOUND` für einen termin-eigenen
Benachrichtigungston (die gewählte Datei wird nach
`~/.local/share/ragnar-raven/sounds/` kopiert — Verschieben oder
Löschen des Originals bricht den Termin nie; ohne eigenen Ton gilt
der globale Benachrichtigungston). CalDAV-kompatibel für die
geplante Nextcloud/ownCloud-Synchronisation. Schreibzugriffe sind
atomar — ein Absturz hinterlässt nie eine halb geschriebene Datei.

Taskbar-Anbindung: markierte Tage im Uhren-Kalender, Rechtsklick-
Menü (Termin eintragen / Benachrichtigungen schalten), Doppelklick
öffnet Raven am gewählten Tag; das Glocken-Applet liest die
gemeinsame history.jsonl.

## Build

```bash
cargo build --release
```

## Layout

- `src/store.rs` — .ics lesen/schreiben, Termin-Modell,
  Lebenszyklus der kopierten Ton-Dateien
- `src/ical.rs` — iCalendar-Helfer (Datum/Zeit parsen/formatieren,
  VALARM-Trigger)
- `src/sidebar.rs` — kompaktes Tagesfenster (Layer-Shell oben rechts)
- `src/dialog.rs` — Termin-Editor (Titel, Notiz, Uhrzeit, Erinnerungs-
  Presets + freier Offset, Ton-Auswahl pro Termin)
- `src/reminder.rs` — Reminder-Daemon-Loop (VALARM-Scheduling)
- `src/card.rs` — die Slide-in-Karte: Kategorie-Akzente, Fenster
  pro Monitor, Karten-Registry, Karten-Töne
- `src/notify.rs` — org.freedesktop.Notifications-Server + Verlauf
- `src/main.rs` — Einstieg, Single-Instance/D-Bus-Verdrahtung
- `src/util.rs` — atomare Datei-Schreibzugriffe
- `src/sound.rs` — libcanberra-FFI für Event-Sounds
- `src/watch.rs` — VolumeMonitor- + apt-Watcher auf demselben Benachrichtigungspfad
- `src/i18n.rs` — gettext-Wrapper (Textdomain `ragnar-raven`)
