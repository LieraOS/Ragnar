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

## Done (recent)

- [x] Bar removal lifecycle fix (0.3.3): `BarState.dead` stops every
      pending timer before `window.destroy()` — removed bars no
      longer resurrect through the fullscreen-hide loop
- [x] Settings layout rework (0.3.4): draggable GtkPaned divider for
      the bar column; start-menu page collapses it to a slim
      "← Leisten" back strip
- [x] Arrangement strip as live bar miniature (0.3.5): dashed stretchy
      gap segments, "+" slots insert gaps in place, drop-between
      targets, arrangement reset — explicit "Füllraum" button gone
- [x] Per-bar animated gradient background (0.3.5): colour list,
      direction (→/←/↓/↑/spin), speed, 15/30/60 fps preset, off by
      default — Cairo layer under the applets
- [x] Volume context menu: "Skald öffnen" → "Lautstärke öffnen" /
      "Open Volume" — still launches ragnar-skald
- [x] Calendar backend resolved (0.3.x): appointments live in Raven —
      the panel marks days from `ragnar-raven`'s appointments.ics and
      double-click/right-click open the Raven editor; no EDS/CalDAV
      needed
- [x] Scrollable settings detail page (0.3.6) — the bar page grew
      past monitor height; it now scrolls instead of overflowing
- [x] Wheel-over-slider forwards to the page scroller (0.3.7) —
      scrolling no longer changes slider values by accident
- [x] Clock context menu slimmed (0.3.8): only "Einrichten …" remains
- [x] Volume scale consistency: ragnar-skald now uses the cubic
      wpctl/pavucontrol convention like the taskbar (fix landed in
      skald, not here)
- [x] Dead `pin_monitor_now` removed (0.3.9) — the async
      `pin_monitor` won
