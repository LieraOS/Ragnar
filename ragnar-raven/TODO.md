# TODO

## Deutsch

- [x] Taskbar-Anbindung: Kalender-Markierungen, Rechtsklick-Menü (Termin eintragen, Benachrichtigungen ein/aus), Doppelklick öffnet Raven am Tag (`--day` via GApplication/D-Bus)
- [x] Reminder-Daemon: VALARM-Zeitpunkte, Slide-in von rechts, Auto-Dismiss
- [ ] Slide-in zur generischen Benachrichtigungs-Komponente ausbauen (Icon/Titel/Text/Dauer) — auch Update- und Systemmeldungen, nicht nur Termine
- [x] Anzeigedauer (`duration_s`) + optionaler Ton (`sound`, freedesktop-Event-ID via canberra) aus settings.ini — ragnar-settings-UI fehlt noch
- [x] Notification-Server (org.freedesktop.Notifications, übernimmt den Namen per REPLACE von ragnar-daemon)
- [x] Verlauf persistent (`history.jsonl`, JSON-Lines, max 200 Einträge, überlebt Neustarts)
- [ ] CalDAV-Sync (Nextcloud/ownCloud) — .ics-Format bereits kompatibel
- [ ] Zentrale Cloud-Zugänge: Credentials verschlüsselt über freedesktop Secret Service (`org.freedesktop.secrets`, z.B. gnome-keyring) hinterlegt — Ragnar Settings schreibt sie, Raven/Valkyre/etc. lesen sie beim Sync
- [ ] Autostart-Eintrag (systemd --user oder .desktop) für den Daemon-Modus
- [ ] Übersetzungen (gettext-Domain `ragnar-raven`), .desktop-Datei, Verpackung als .deb
- [ ] Raven aus ragnar-daemon ablösen

## English

- [x] Taskbar integration: calendar markers, right-click menu (add appointment, notifications on/off), double-click opens Raven at that day (`--day` via GApplication/D-Bus)
- [x] Reminder daemon: VALARM times, right-edge slide-in, auto-dismiss
- [ ] Grow the slide-in into the generic notification component (icon/title/body/duration) — updates and system messages, not only appointments
- [x] Display duration (`duration_s`) + optional sound (`sound`, freedesktop event id via canberra) from settings.ini — ragnar-settings UI still missing
- [x] Notification server (org.freedesktop.Notifications, takes the name via REPLACE from ragnar-daemon)
- [x] Persisted history (`history.jsonl`, JSON lines, max 200 entries, survives restarts)
- [ ] CalDAV sync (Nextcloud/ownCloud) — .ics format already compatible
- [ ] Central cloud credentials: stored encrypted via the freedesktop Secret Service (`org.freedesktop.secrets`, e.g. gnome-keyring) — Ragnar Settings writes them, Raven/Valkyre/etc. read them at sync time
- [ ] Autostart entry (systemd --user or .desktop) for daemon mode
- [ ] Translations (gettext domain `ragnar-raven`), .desktop file, .deb packaging
- [ ] Replace Raven in ragnar-daemon
