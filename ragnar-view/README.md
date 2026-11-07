# ragnar-view

## Deutsch

Desktop-Icons-Schicht für den Ragnar-Desktop (LieraOS). Ersetzt den
alten legacy/`ragnar-desktop-view` vollständig durch Rust + GTK4 +
gtk4-layer-shell — eine `Layer::Bottom`-Surface **pro Monitor**,
unterhalb aller Fenster. Jedes Icon gehört genau einem Ausgang:
die geteilte Layout-Map (Anschluss → Positionen in
`~/.config/ragnar-view/layout-<Anschluss>`) entscheidet, wo es
erscheint. Der Primärmonitor übernimmt alles Unzugewiesene
(inkl. Icons abgesteckter Monitore); die alte Datei `layout`
wird als seine Sektion geerbt.

Icons lassen sich über Monitorgrenzen ziehen — beim Drop gehört
das Icon dem Zielmonitor. „Neuer Ordner" / „Leeres Dokument"
landet am Klickpunkt auf dem Monitor, auf dem das Menü geöffnet
wurde.

Zeigt: Persönlicher Ordner, Papierkorb, eingebundene Laufwerke
(gio::VolumeMonitor) und den Inhalt von ~/Desktop.

Kontextmenüs (rechts lokalisiert, de/en):

- **Hintergrund:** Leeres Dokument / Neuer Ordner anlegen,
  Sortieren (Name, Datum, Größe, Typ), Im Dateimanager öffnen,
  Hintergrund / Anzeige / System → `ragnar-settings`.
- **Icon:** Öffnen, Öffnen mit… (empfohlene + alle Programme),
  Im Terminal öffnen, Umbenennen, Löschen. Löschen-Verhalten
  (endgültig mit Rückfrage = Standard, oder Papierkorb) und
  sichtbare Einträge stellt der Benutzer in Ragnar Settings →
  „Panel und Desktop" ein (GSettings `org.lieraos.ragnar.view`).
- Icons frei verschiebbar (Drag & Drop), Positionen bleiben
  gespeichert (`~/.config/ragnar-view/layout`). Untermenü
  „Symbolanordnung": Am Raster ausrichten (Standard), Frei
  platzieren, Automatisch anordnen. „Sortieren nach" (Name, Datum,
  Größe, Typ) ordnet die Icons einmalig neu ins Raster.
- Menüs zeigen immer vollständig, ohne Scrollen (GMenu-Sections
  lassen GTKs PopoverMenu ~13 px pro Section falsch messen — die
  Menüs sind deshalb flach aufgebaut).
- Mehrfachauswahl per Gummiband; markierte Icons lassen sich
  gemeinsam verschieben.
- Dialoge schließen sich bei Klick auf den freien Desktop.
- Hauptmonitor = `org.lieraos.ragnar.panel primary-monitor-list`.

Aktualisiert sich automatisch über `gio::FileMonitor` auf
~/Desktop und VolumeMonitor-Signale.

## English

Desktop icons layer for the Ragnar desktop (LieraOS). Fully
replaces the old legacy `ragnar-desktop-view` with Rust + GTK4 +
gtk4-layer-shell — one `Layer::Bottom` surface **per monitor**
beneath all windows. Every icon belongs to exactly one output:
the shared layout map (connector → positions in
`~/.config/ragnar-view/layout-<connector>`) decides where it
shows. The primary monitor claims everything unassigned
(including icons of unplugged monitors); the old `layout` file
is inherited as its section.

Icons can be dragged across monitor edges — on drop the icon
belongs to the target output. "New Folder" / "Empty Document"
land at the click point on the monitor the menu was opened on.

Shows: Home, Trash, mounted volumes (gio::VolumeMonitor) and the
contents of ~/Desktop.

Context menus (localized de/en):

- **Background:** New empty document / folder, sorting (name,
  date, size, type), Open in file manager, Background / Display /
  System → `ragnar-settings`.
- **Icon:** Open, Open with… (recommended + all apps), Open in
  terminal, Rename, Delete. Delete behaviour (permanent with
  confirmation = default, or trash) and visible entries are set by
  the user in Ragnar Settings → "Panel and desktop" (GSettings
  `org.lieraos.ragnar.view`).
- Icons can be dragged freely, positions are remembered
  (`~/.config/ragnar-view/layout`). "Icon Arrangement" submenu:
  Snap to Grid (default), Free Placement, Auto Arrange. "Sort by"
  (name, date, size, type) re-flows the icons into the grid once.
- Menus always show in full without scrolling (GMenu sections
  make GTK's popover menu mismeasure ~13 px per section — the
  menus are therefore laid out flat).
- Rubber-band multi-selection; selected icons move together.
- Dialogs close when the bare desktop is clicked.
- Main monitor = `org.lieraos.ragnar.panel primary-monitor-list`.

Refreshes automatically via `gio::FileMonitor` on ~/Desktop and
VolumeMonitor signals.
