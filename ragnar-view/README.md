# ragnar-view

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

Filename effects (optional; off = identical rendering): icon
captions are drawn by the `FxLabel` widget, which live-reads
`~/.config/ragnar-settings/font-effects.ini` [names] via a file
monitor — either a brightness pulse or a two-colour wave
travelling across the glyphs. Colours, direction, strength and
speed are configured in Ragnar Settings → Appearance → Fonts.
