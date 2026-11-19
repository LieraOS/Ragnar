# ragnar-software

Software-Verwaltung für den Ragnar Desktop — GPL-3.0-or-later,
Teil der Ragnar-Familie. English version: [README.md](README.md)

GTK4/libadwaita-Anwendung mit fünf Bereichen in der Sidebar:

- **Willkommen** — Update-Status im Hintergrund
- **Updates** — auswählbare Paketliste, Schweregrade, Changelog-Panel,
  Abhängigkeits-Vorschau, Fortschritts-Dialog; deckt apt-Pakete
  (PackageKit `RefreshCache` + `GetUpdates`) und Flatpak-Updates
  (Flathub, User-Scope) ab
- **Software** — AppStream-Katalog (Debian DEP-11 + Flathub-XML),
  Kategorien, Suche, Quellen-Filter (Alle/Debian/Flathub),
  Quellen-Badge, App-Icons, App-Detail-Panel, Installieren + Deinstallieren
- **Verlauf** — Transaktionshistorie (apt history.log + flatpak history)
- **Einstellungen** — Flathub-Remote an/aus, Quellen-Dialog:
  deb822/.list-Quellen editierbar, entfernen, neu hinzufügen;
  pending-Changes, ein pkexec beim Anwenden
- **i18n** — de/en über gettext; englische Quellstrings,
  deutscher Katalog in `po/de.po` (Englisch = msgid-Fallback)

Zusätzlich: Doppelklick-Installation lokaler `.deb`- und
`.flatpakref`-Dateien (MIME-Handler, `InstallFiles` /
`flatpak install --from`).

Backend: PackageKit über den System-D-Bus (zbus) — Transaktionen,
polkit (`auth_admin_keep`, ein Passwort pro Sitzung) und Fortschritt
liefert packagekitd. Flatpak läuft über die flatpak-CLI im
User-Scope (kein Passwort).

## Build

```bash
cargo fmt -- --check && cargo clippy && cargo build --release
cp target/release/ragnar-software pkg/ragnar-software/usr/bin/
dpkg-deb --build pkg/ragnar-software ragnar-software_0.1.0_amd64.deb
```

## Laufzeit-Abhängigkeiten

`packagekit`, `flatpak`, `libadwaita-1-0`, `libgtk-4-1`
(AppStream-Daten kommen via `apt update` / `flatpak update --appstream`).

Siehe TODO.md für die Roadmap.
