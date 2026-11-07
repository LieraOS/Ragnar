# ragnar-taskbar

Wayland-native, GTK4-based taskbar for the Ragnar desktop (LieraOS).
Written in Rust; rendered through `wlr-layer-shell` via gtk4-layer-shell.

## Features

- Classic taskbar layout: start menu left, task list centre,
  tray + clock right — as a freely ordered applet sequence with
  flexible or fixed-width spacers (`menu;spacer;tasklist;spacer;tray;status;clock`)
- Task list: real running windows via `zwlr_foreign_toplevel_manager_v1`
  (activate/minimize on left-click; right-click → minimize /
  maximize / close)
- Workspace switcher: real workspaces via `ext-workspace-v1`
  (number buttons, active highlighted, click → activate); the
  settings page sets both the compositor's workspace count
  (writes labwc's `<desktops number>` + live reconfigure) and a
  per-bar cap on shown buttons
- System tray: StatusNotifier host against ragnar-daemon's watcher —
  real items incl. ayatana paths and IconThemePath icons (Discord,
  Steam, Blueman, Heimdall); right-click → the app's own
  `com.canonical.dbusmenu` menu with real cascading submenus, item
  icons (icon-name + icon-data), check/radio toggles, separators
  and live layout updates via LayoutUpdated
- Clock: left-click → Cinnamon-style calendar panel (date, upcoming
  events placeholder, month calendar, "Datums- und
  Zeiteinstellungen"); right-click → applet context menu
  (Über / Einrichten / »Kalender« entfernen)
- Task-button hover → live window thumbnail preview via
  ext-image-copy-capture (title + scaled texture over the applet)
- Multi-bar: bars are spawned/managed at runtime from a registry;
  the settings window edits every bar independently (edge incl.
  vertical left/right, geometry, applets, opacity, colour)
- Categorised start menu (hover-switchable GtkStack, search,
  session actions, Ragnar logo) — categories and per-app
  assignments are user-editable via menu.ini and the settings page
  (drag & drop onto categories, per-app rename/hide, hover vs click
  switching, custom categories)
- Hardware-sensor applets (CPU/GPU/RAM/motherboard temps) read
  sysfs hwmon directly — nvidia-smi fallback for proprietary GPUs;
  custom `cmd:` applets run user shell commands and show stdout
- Menus are single fullscreen layer-shell overlay surfaces —
  outside click or ESC dismisses, no input grabs, positioned above
  the clicked applet
- Right-click on empty bar space → "Eigenschaften" opens the
  settings window: bars on four edges incl. vertical, height,
  width, alignment, icon size, opacity + colour per bar,
  drag-and-drop applet ordering, per-bar applet checkboxes,
  custom cmd: applets
- Per-bar config in `~/.config/ragnar-taskbar/bars.ini` — one
  `[bar-N]` section per bar; built for multiple bars
- The standalone Ragnar panel

## i18n

UI is gettext-based: German is the msgid language, `po/en.po`
provides English (`/usr/share/locale/…/ragnar-taskbar.mo`).
`cargo test` covers the pure parsers (rc.xml desktops, cmd:
specs, workspace label shortening).

## Build

```sh
sudo apt install libgtk4-layer-shell-dev
cargo build --release
./target/release/ragnar-taskbar
```

Runtime dependency: `libgtk4-layer-shell0`.
`wayland-client` uses runtime loading (`dlopen`) so libwayland never
precedes gtk4-layer-shell in link order — that order would silently
break layer-shell support.

## German

# ragnar-taskbar

Wayland-native Taskbar für den Ragnar-Desktop (LieraOS), in Rust mit
GTK4, über `wlr-layer-shell` gerendert.

- Klassisches Layout: Startmenü links, Taskliste Mitte, Tray + Uhr
  rechts — als frei sortierbare Applet-Reihenfolge mit flexiblen oder
  festen Abstandshaltern
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
- Uhr: Linksklick → Kalender-Panel im Cinnamon-Stil; Rechtsklick →
  Applet-Kontextmenü (Über / Einrichten / entfernen)
- Hover über Task-Buttons → Live-Fenster-Vorschau (Thumbnail) via
  ext-image-copy-capture
- Multi-Leiste: Leisten werden zur Laufzeit aus einer Registry
  verwaltet; das Einstellungsfenster bearbeitet jede Leiste getrennt
  (Kante inkl. vertikal, Geometrie, Applets, Transparenz, Farbe)
- Kategorisiertes Startmenü (Hover-Stack, Suche, Session-Aktionen,
  Ragnar-Logo) — Kategorien und App-Zuordnung sind in menu.ini und
  über die Einstellungsseite frei bearbeitbar (Drag&Drop auf
- Sensor-Applets (CPU/GPU/RAM/Mainboard) lesen sysfs-hwmon direkt —
  nvidia-smi-Fallback für proprietäre GPUs; eigene cmd:-Applets
  führen Shell-Befehle aus und zeigen deren Ausgabe
  Kategorien, App umbenennen/ausblenden, Hover-/Klick-Modus,
  eigene Kategorien)
- Menüs sind einzelne Fullscreen-Layer-Shell-Overlay-Surfaces —
  Außenklick oder ESC schließt, keine Input-Grabs
- Rechtsklick auf freie Leistenfläche → Leistenmenü mit
  Live-Eigenschaften: Kante, Höhe, Breite, Ausrichtung,
  Icon-Größe, visuelle Drag-and-Drop-Layout-Vorschau
- Pro Leiste eine `[bar-N]`-Sektion in
  `~/.config/ragnar-taskbar/bars.ini`
- Die Ragnar-Leiste
