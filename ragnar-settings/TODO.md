# Ragnar Settings TODO

## Phase A — skeleton

- [x] Cargo workspace: GTK4 + libadwaita app, English code,
      i18n from day one, files <= 400 lines, one module
      per feature area.
- [x] Hub window: categorized tile grid + search field, registry
      catalog as the category/tile model.
- [x] Panel launcher abstraction: embedded panel vs. external app tile
      (`Action::Panel` / `Action::Launch`).
- [x] First real panel: "About this system" (os-release, kernel,
      memory, desktop session).
- [x] Availability checks per tile; unavailable entries are hidden so
      the app runs on non-Ragnar dev machines.
- [x] .desktop file + icon — shipped since 1.0.4, appears under
      "Einstellungen" in the taskbar menu.
- [x] gettext catalogs + workflow (1.0.12): `po/de.po` complete
      (495 strings, msgfmt-clean), `po/ragnar-settings.pot` template,
      `build.rs` compiles every po/*.po into `target/locale/` and
      cargo-deb ships the `.mo` under `/usr/share/locale/`;
      `LOCALEDIR` override for dev runs; extraction/merge commands
      documented in README. English is the source language — no
      en.po needed. es/fr catalogs open for later.

## Phase B — native panels

- [x] Personalization: appearance/ panel — mode (color-scheme +
      panel dark-theme), accent colors, theme/cursor rows.
- [x] Animated wallpaper switch (beta) in the wallpaper panel —
      writes ~/.config/ragnar-settings/wallpaper-animated; the
      ragnar-wallpaper daemon then spawns ragnar-livewall instead
      of swaybg.
- [x] Effects editor (1.0.16): "Effects…" in the wallpaper panel
      opens a dialog with a live GL mini-preview running the same
      installed GLSL as ragnar-livewall; stars + mist regions are
      dragged directly on the preview, parameters via sliders;
      Save writes livewall-effects.ini which ragnar-wallpaper
      applies instantly.
- [x] Glow as effect + multi-regions (1.0.17): the aurora band
      (shown as "Glow" in the UI) is a toggleable effect with
      region/intensity like the others;
      every effect accepts up to four drag-rects (marching-ants
      highlight), per-effect Reset button, snow/rain/fireflies/cloud
      shadows added — the dialog renders all seven shader blocks.
- [ ] Extend the editor: lamp-glow regions — same mechanism,
      needs livewall shader support first.
- [x] Display: display/ panel (canvas arrangement, resolution,
      orientation, GPU section).
- [x] Sounds: sounds.rs writes ragnar-raven's settings.ini —
      raven reads it as its interface.
- [x] Corrupt ini writer fixed (1.0.10): sound/notification writes go
      through the tested panels::ini::write_key.
- [x] Notifications panel extended (1.0.11): Events card toggles
      Raven's watchers, Colours card writes per-category color_*
      keys; OSD rows moved to panels/osd.rs.
- [x] Night light: gammastep-backed panel.
- [x] Updates: pending count via apt simulation, pkexec
      refresh/install, auto-upgrade drop-in with explicit
      origin=Debian,label=Debian pattern.
- [x] Accessibility: text size, reduce animations, high contrast —
      all schema-guarded.
- [x] Shared helpers: panels/common.rs + generic INI panels/ini.rs.
- [x] Removable media: automount switch + live "Connected devices"
      list with unmount/eject via gio::VolumeMonitor.
- [x] Autostart view in desktop.rs.
- [x] Default applications (xdg-mime); extended with calendar
      and archive categories, category labels now actually run
      through gettext (1.0.12).
- [ ] Migrate old panels (privacy, power, media, ...) to
      panels::common helpers.
- [x] Power-panel crash fixed (1.0.12): unguarded access to
      org.gnome.settings-daemon.plugins.power (gsd is not part of
      Ragnar) aborted the whole app on open; rows are now
      schema-gated. Profile switching was moved from the
      powerprofilesctl CLI to the net.hadess.PowerProfiles D-Bus
      API — works identically against power-profiles-daemon or
      tuned-ppd (tuned-ppd added to the ISO's
      performance.list.chroot so tuned stays the tuning engine).
- [x] Software tile launches ragnar-software (1.0.12);
      printers panel rewritten — lists configured queues with
      default/remove, no add UI (drivers own that); standard
      PDF printer auto-provisioned via CUPS-PDF with editable
      per-user defaults (paper size, resolution, PDF version).
- [x] Terminal argv simplified (1.0.12): dropped the per-emulator
      special cases — gnome-terminal/kgx use `--`, everything else
      gets the portable `-e sh -c` form.
- [x] On-screen keyboard / screen reader rows (1.0.12,
      experimental section in Accessibility): write the real
      a11y.applications flags and report whether a consumer
      (squeekboard/onboard/orca) is installed — missing tools
      are pointed at Software with an "Open Software" jump
      button.
- [x] Panel & Desktop: org.lieraos.ragnar.* settings are exposed —
      per-panel options (position, autohide, applet defaults) via
      ragnar-taskbar's own settings window (no duplicate UI here by
      design); notification-position added to the notifications
      panel (1.0.12).

## Phase C — normal vs. advanced

- [x] Advanced mode (1.0.12): persistent switch in the header bar,
      state in `~/.config/ragnar-settings/settings.ini`
      (`[ui] advanced`); tile-level hiding via
      `registry::catalog::ADVANCED_TILES`, section-level guards via
      `crate::advanced::enabled()` inside panels (GPU controls in
      Display, rc edit in Input, group management in Users).
- [x] Search focus fix (1.0.12): the hub rebuild on every keystroke
      no longer runs the stale-pointer-grab workaround, so the entry
      keeps keyboard focus for continuous typing.
- [x] Module splits for the line budget (1.0.12):
      display/canvas.rs → monitor_rect.rs (rects, drag/select,
      thumbnails), registry/catalog.rs → entries.rs (tile list).
- [x] All clippy warnings resolved; `cargo test`, `msgfmt --check`
      and release build green.
- [x] Compositor panel (1.0.12, advanced tile): window decoration
      (corner radius, drop shadows, keep-border, titlebar layout +
      font), window gap, work-area margins, workspace count + popup
      time, focus behaviour, snap distances, top-maximize, placement
      policy, tearing — written live to the compositor configuration
      file. "Coming with the native compositor" section lists locked
      rows (blur, animations, tiling layouts, gestures, window
      groups, scratchpad, mirroring).
- [x] Compositor capability layer (1.0.12): `backend/compositor.rs`
      detects the running compositor (labwc / native Ragnar /
      unknown) and maps per-feature support; locked rows unlock
      automatically once the native compositor provides the
      capability — no second UI implementation.
- [x] Generic rc.xml patcher (1.0.12): `panels/rcxml.rs` replaces
      the input-only `rc_edit` helper — block get/set for any
      section, nested blocks via parent, attribute read/write
      incl. attribute-keyed elements (`<font place="…">`),
      auto-creation of missing sections/elements, `labwc -r` reload;
      input + shortcuts panels migrated; fixture-tested.
- [x] Window rules editor (1.0.12, advanced tile, experimental):
      list/add/edit/delete for the compositor's window rules —
      matchers (app id, title, first-match-only), window-state
      actions (top/below, omnipresent, maximized, fullscreen,
      minimized), placement (snap edge, center, workspace),
      titlebar override and behaviour flags. Round-trip model
      preserves unmodelled attributes/actions/comments verbatim;
      fixture-tested.
- [x] Keyboard-shortcut editor v2 (1.0.12): every compositor
      keybind is now editable — key-capture dialog plus a WM-action
      catalogue (run command, close/kill/minimize, maximize,
      fullscreen, snap-to-edge, go/move-to-workspace, window
      switching, menus, desktop, reconfigure). Session-managed
      binds (managed="labwc-bridge") are shown read-only since the
      bridge regenerates them from GSettings.
- [x] Workspace names (1.0.12, compositor panel): per-workspace
      name entries writing <desktops><names> — empty slots keep the
      generated "<prefix> N" label; entry count follows the
      workspace-count spin live.
- [x] Autostart & environment (1.0.12, advanced tile): editors
      for the session's autostart file (programs at login) and
      environment variables — in-place editing preserving comments,
      session-managed variables shown read-only.
- [x] Input depth (1.0.12): double-click time, tap-and-drag,
      drag lock, three-finger drag, tap button map, click method,
      scroll method, scroll speed factor and per-window keyboard
      layout scope added to the input panel; invalid "keep"
      num-lock value removed (the compositor only accepts
      on/off). Touchpad-only rows stay hidden without a
      detected touchpad.
- [x] Notification position (1.0.12, notifications panel):
      screen-corner dropdown writing the panel schema's
      notification-position enum, schema-guarded so the row
      only appears where the schema is installed.
- [x] Configuration backup & restore (1.0.12): footer buttons open
      file dialogs; export stages every source from the declarative
      manifest in `src/backup/mod.rs` (~/.config files + dirs,
      whole-schema gsettings dumps incl. every org.lieraos.ragnar.*
      schema found at runtime, system files) into one timestamped
      .tar.gz; restore replays files + gsettings and installs
      system files through a single pkexec script. New config
      sources register as one manifest line — the feature grows
      with the app by design.

## Phase D — launched-app tiles

- [x] Sound tile → Liera Audio (`lieraaudio`, verified in catalog).
- [x] Network & Bluetooth tile → Heimdall (`ragnar-heimdall`).
- [x] Software tile → launches ragnar-software.

## Phase E — adoption

- [x] Debian packaging — cargo-deb ships binary + .desktop + .mo.
- [x] In ISO package list (packages.chroot staging).
- [x] ragnar-control-center removed from the ISO — ragnar-settings
      is the only settings hub.
- [ ] Automated tests beyond ini/apt/terminal/search (display
      dbus_client parsing, advanced-mode filtering).
