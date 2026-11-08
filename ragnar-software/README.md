# ragnar-software

Software-Verwaltung für den Ragnar Desktop / software management
for the Ragnar desktop — GPL-3.0-or-later, Teil der Ragnar-Familie.

## Deutsch

GTK4/libadwaita-Anwendung mit fünf Bereichen in der Sidebar:

- **Willkommen** — Update-Status im Hintergrund
- **Updates** — auswählbare Paketliste, Schweregrade, Changelog-Panel,
  Abhängigkeits-Vorschau, Fortschritts-Dialog
- **Software** — AppStream-Katalog (Debian DEP-11 + Flathub-XML),
  Kategorien, Suche, Quellen-Filter (Alle/Debian/Flathub),
  Quellen-Badge, App-Detail-Panel mit Beschreibung,
  Installieren + Deinstallieren
- **Verlauf** — Transaktionshistorie (apt history.log + flatpak history)
- **Einstellungen** — Flathub-Remote an/aus, Quellen-Dialog:
  deb822/.list-Quellen editierbar, entfernen, neu hinzufügen;
  pending-Changes, ein pkexec beim Anwenden
- **i18n** — de/en über gettext

Zusätzlich: Doppelklick-Installation lokaler `.deb`- und
`.flatpakref`-Dateien (MIME-Handler, `InstallFiles` /
`flatpak install --from`).

Backend: PackageKit über den System-D-Bus (zbus) — Transaktionen,
polkit (`auth_admin_keep`, ein Passwort pro Sitzung) und Fortschritt
liefert packagekitd. Flatpak läuft über die flatpak-CLI im
User-Scope (kein Passwort).

## English

GTK4/libadwaita application with five sidebar pages:

- **Welcome** — background update status
- **Updates** — selectable list, severities, changelog pane,
  dependency preview, progress dialog
- **Software** — AppStream catalogue (Debian DEP-11 + Flathub XML),
  categories, search, source filter, badges, app detail pane,
  install + remove
- **History** — transaction log (apt history.log + flatpak history)
- **Settings** — Flathub toggle, sources dialog: editable
  deb822/.list entries, remove, add; pending changes, single
  pkexec on apply
- **i18n** — de/en via gettext

Plus double-click install of local `.deb`/`.flatpakref` files.

Backend: PackageKit over the system D-Bus (zbus); flatpak via CLI
in user scope.

## Build

```bash
cargo fmt -- --check && cargo clippy && cargo build --release
dpkg-deb --build pkg/ragnar-software ragnar-software_0.0.1_amd64.deb
```

## Runtime dependencies

`packagekit`, `flatpak`, `libadwaita-1-0`, `libgtk-4-1`
(AppStream-Daten kommen via `apt update` / `flatpak update --appstream`).

Siehe TODO.md für die Roadmap. / See TODO.md for the roadmap.
