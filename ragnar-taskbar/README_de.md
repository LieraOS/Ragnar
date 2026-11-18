# ragnar-taskbar

Wayland-native Taskbar für den Ragnar-Desktop (LieraOS), in Rust mit
GTK4, über `wlr-layer-shell` gerendert.

## Features

- Klassisches Layout: Startmenü links, Taskliste Mitte, Tray + Uhr
  rechts — als frei sortierbare Applet-Reihenfolge mit flexiblen oder
  festen Abstandshaltern, bearbeitet als Live-Miniatur der Leiste:
  Abstände erscheinen als gestrichelte dehnbare Segmente, „+"-Slots
  fügen sie an Ort und Stelle ein, Chips sortieren per Drag & Drop,
  ein Klick setzt die Anordnung zurück
- Optionaler animierter Verlauf-Hintergrund pro Leiste (Standard:
  aus): eigene Farbliste, Flussrichtung (→/←/↓/↑/rotierend), Tempo
  und Glätte-Preset 15/30/60 fps — als Cairo-Verlaufsebene unter den
  Applets gezeichnet, null Overhead solange deaktiviert
- Taskliste: echte laufende Fenster über
  `zwlr_foreign_toplevel_manager_v1` (Linksklick aktiviert;
  Rechtsklick → Minimieren/Maximieren/Schließen)
- Arbeitsflächen-Umschalter: echte Workspaces via
  `ext-workspace-v1` (Zahlen-Buttons, aktive markiert, Klick →
  Umschalten); die Einstellungen steuern sowohl die Gesamtzahl
  (schreibt labwcs `<desktops number>` + lädt live neu) als auch
  die pro Leiste angezeigte Höchstzahl
- System-Tray: StatusNotifier-Host am Watcher von ragnar-daemon —
  echte Items inkl. ayatana-Pfaden und IconThemePath-Icons;
  Rechtsklick → das eigene `com.canonical.dbusmenu`-Menü der App
  mit echten kaskadierenden Untermenüs, Item-Icons (icon-name +
  icon-data), Check-/Radio-Toggles, Separatoren und Live-Updates
  über LayoutUpdated
- Uhr: Linksklick → Kalender-Panel im Cinnamon-Stil (Datum, laufende
  Uhrzeit, Monatskalender, „Datums- und Zeiteinstellungen");
  Rechtsklick → „Einrichten …"; Tage mit Terminen werden direkt aus
  Ravens appointments.ics markiert, Doppelklick oder Rechtsklick auf
  einen Tag öffnet den Raven-Editor
- Hover über Task-Buttons → Live-Fenster-Vorschau (Thumbnail) via
  ext-image-copy-capture
- Multi-Leiste: Leisten werden zur Laufzeit aus einer Registry
  verwaltet; das Einstellungsfenster bearbeitet jede Leiste getrennt
  (Kante inkl. vertikal, Geometrie, Applets, Transparenz, Farbe)
- Kategorisiertes Startmenü (Hover-Stack, Suche, Session-Aktionen,
  Ragnar-Logo) — Kategorien und App-Zuordnung sind in menu.ini und
  über die Einstellungsseite frei bearbeitbar (Drag&Drop auf
  Kategorien, App umbenennen/ausblenden, Hover-/Klick-Modus,
  eigene Kategorien)
- Sensor-Applets (CPU/GPU/RAM/Mainboard) lesen sysfs-hwmon direkt —
  nvidia-smi-Fallback für proprietäre GPUs; eigene cmd:-Applets
  führen Shell-Befehle aus und zeigen deren Ausgabe
- Menüs sind einzelne Fullscreen-Layer-Shell-Overlay-Surfaces —
  Außenklick oder ESC schließt, keine Input-Grabs
- Rechtsklick auf freie Leistenfläche → Leistenmenü mit
  Live-Eigenschaften: Kante, Höhe, Breite, Ausrichtung,
  Icon-Größe, visuelle Drag-and-Drop-Layout-Vorschau; die
  Leisten-Spalte ist als GtkPaned-Teiler frei breitenziehbar und
  klappt auf der Startmenü-Seite zu einem schmalen Zurück-Streifen
  ein, damit der Menü-Manager die volle Fensterbreite bekommt; die
  Detailseite scrollt vertikal, und das Mausrad über einem Regler
  scrollt die Seite statt den Wert zu ändern
- Benachrichtigungs-Glocke: reiner Leser von ragnar-ravens
  gemeinsamer history.jsonl — nur sichtbar, solange ungelesene
  Einträge existieren, das Öffnen der Liste markiert sie als gesehen
- Pro Leiste eine `[bar-N]`-Sektion in
  `~/.config/ragnar-taskbar/bars.ini`
- Die eigenständige Leiste des Ragnar-Desktops

## i18n

Die UI basiert auf gettext: Deutsch ist die msgid-Sprache, `po/en.po`
liefert die englischen Übersetzungen; `build.rs` kompiliert die .mo
beim Build und installiert sie unter share/locale.

## Build

```sh
cargo build --release
./target/release/ragnar-taskbar
```

Laufzeit-Abhängigkeit: `libgtk4-layer-shell0`.
`wayland-client` nutzt Laufzeit-Laden (`dlopen`), damit libwayland
nie vor gtk4-layer-shell in der Link-Reihenfolge steht — diese
Reihenfolge würde die Layer-Shell-Unterstützung still brechen.
