# ragnar-taskbar TODO

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
- Applet context menus only get an "Einrichten …" entry where the
  applet has its own settings (clock has it); Über/entfernen were
  deliberately dropped — applet management lives in the bar settings
- Menus NEVER use GtkPopover on layer surfaces — always the
  overlay-surface primitive in `src/menu.rs`
- `wayland-client` must stay dlopen-linked — a hard link puts
  libwayland before gtk4-layer-shell and breaks layer-shell init

