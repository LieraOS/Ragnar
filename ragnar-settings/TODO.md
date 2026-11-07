# Ragnar Settings TODO

## Preparation (done)

- [x] Create project directory — development starts after the current
      live image with the Phase-2 rebrand is built and boot-tested.

## Phase A — skeleton

- [x] Cargo workspace: GTK4 + libadwaita app, English code,
      i18n from day one (de/en/es/fr), files <= 400 lines, one module
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
- [ ] gettext catalogs (de/en/es/fr) + xgettext extraction workflow.

## Phase B — first native panels

- [x] Personalization: appearance/ panel — mode (color-scheme +
      panel dark-theme), accent colors, theme/cursor rows.
- [x] Display: display/ panel exists (resolution/orientation).
- [x] Sounds: sounds.rs writes ragnar-raven's settings.ini —
      raven reads it as its interface.
- [x] Night light: gammastep-backed panel (config.ini + managed
      autostart; no sunset mode — no location service installed).
      Nachtlicht-Panel via gammastep.
- [x] Updates: pending count via apt simulation, pkexec
      refresh/install, auto-upgrade switch writing
      /etc/apt/apt.conf.d/52ragnar-auto-upgrades with an explicit
      origin=Debian,label=Debian pattern (lsb_release -cs = "n/a"
      makes the shipped ${distro_codename} patterns match nothing).
      Updates-Panel mit Auto-Update-Schalter.
- [x] Accessibility: text size, reduce animations, high contrast —
      all schema-guarded. Barrierefreiheit-Panel.
- [x] Shared helpers: panels/common.rs + generic INI
      panels/ini.rs. Gemeinsame Panel-Helfer.
- [ ] Migrate old panels (privacy, power, media, notifications, ...)
      to panels::common helpers and notifications.rs' private
      write_key to panels::ini. Alte Panels auf common.rs/ini.rs
      migrieren.
- [ ] Software & Printers panels still open (deferred by user).
      Software- und Drucker-Panels noch offen.
- [ ] Verify terminal argv conventions for alacritty/kitty/tilix etc.
      on a machine where they are installed — unit-tested but only
      gnome-terminal exists here. Terminal-Argv-Konventionen
      testabgedeckt, aber nur gnome-terminal installiert.
- [ ] On-screen keyboard / screen reader rows for Accessibility once
      such tools are installed — currently nothing to control.
      Bildschirmtastatur-/Screenreader-Reihen, sobald installiert.
- [x] Removable media: automount switch
      (`org.lieraos.ragnar.automount enabled`, schema shipped in this
      package, compiled via postinst) + live "Connected devices" list
      with unmount/eject via gio::VolumeMonitor.
      Panel „Wechseldatenträger": Automount-Schalter + Live-Geräteliste.
- [x] Autostart view in desktop.rs — lists /etc/xdg/autostart +
      user autostart; only user-owned entries are removable.
- [x] Default applications (xdg-mime) — default_apps.rs; fixed a
      crash caused by a nonexistent `org.lieraos.ragnar-view` schema
      and a dead terminal row (1.0.5).
- [ ] Panel & Desktop: expose org.lieraos.ragnar.* settings
      (panel positions, autohide, applet defaults).

## Phase C — launched-app tiles

- [x] Sound tile → Liera Audio (`lieraaudio`, verified in catalog).
- [x] Network & Bluetooth tile → Heimdall (`ragnar-heimdall`, verified in catalog).
- [ ] Software tile → installed-app list + launch Liera Software Center.

## Phase D — adoption

- [x] Debian packaging — cargo-deb ships binary + .desktop since 1.0.3.
- [x] In ISO package list (packages.chroot staging).
- [x] ragnar-control-center removed from the ISO — ragnar-settings
      is the only settings hub.
- [ ] Automated tests (unit + integration), docs, translations.
