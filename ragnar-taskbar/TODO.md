# ragnar-taskbar TODO

## Open

- [ ] Calendar panel: real appointments backend (EDS / CalDAV —
      evaluate which source LieraOS will use)
- [ ] Applet context menu pattern for the remaining applets
      (Über / Einrichten / entfernen — clock has it as reference)
- [ ] Volume applet interaction — wait until LieraSound is finished

## Design rules

- Compositor-agnostic: only standard Wayland protocols
  (layer-shell, ext-*, zwlr_foreign_toplevel) and own D-Bus APIs —
  no labwc-specific calls
- One documented exception: `src/workspace/desktops.rs` writes
  labwc's rc.xml `<desktops number>` because ext-workspace's
  create_workspace is not advertised by labwc; a future compositor
  replaces that file only
- Applets are self-contained widgets registered in `src/applets/`;
  bar layout is an ordered sequence (applets + spacers)
- Menus NEVER use GtkPopover on layer surfaces — always the
  overlay-surface primitive in `src/menu.rs`
- `wayland-client` must stay dlopen-linked — a hard link puts
  libwayland before gtk4-layer-shell and breaks layer-shell init

## German

## Offen

- [ ] Kalender-Panel: echtes Termin-Backend (EDS / CalDAV — Quelle
      für LieraOS festlegen)
- [ ] Applet-Kontextmenü-Muster (Über/Einrichten/entfernen) auf die
      übrigen Applets übertragen
- [ ] Lautstärke-Interaktion — erst nach Fertigstellung von
      LieraSound

## Design-Regeln

- Compositor-agnostisch: nur Standard-Wayland-Protokolle und eigene
  D-Bus-APIs — keine labwc-spezifischen Aufrufe
- Eine dokumentierte Ausnahme: `src/workspace/desktops.rs`
  schreibt labwcs `<desktops number>` in der rc.xml, weil labwc
  ext-workspaces create_workspace nicht anbietet; ein künftiger
  Compositor ersetzt nur diese Datei
- Applets sind eigenständige Widgets in `src/applets/`; Leistenlayout
  ist eine geordnete Sequenz (Applets + Abstandshalter)
- Menüs NIEMALS als GtkPopover auf Layer-Surfaces — immer das
  Overlay-Primitiv in `src/menu.rs`
- `wayland-client` bleibt dlopen-gelinkt — hartes Linken stellt
  libwayland vor gtk4-layer-shell und bricht die Layer-Shell-Init
