# Ragnar TODO

Deutsche Version: [TODO_de.md](TODO_de.md)

Goal reached: Ragnar runs completely free of fork legacy — every
component is a standalone Rust crate or script, individually
replaceable, using neutral interfaces (Wayland protocols, D-Bus).

## Done (verified on the production system)

- **`ragnar-session` 2.x (Rust)** — fully replaces the session
  manager: `org.gnome.SessionManager` API, RegisterClient/
  ClientPrivate handshake, required components, EndSessionDialog
  orchestration, inhibit API, `ragnar-session.target` activates the
  generated autostart tree (stop+start on every login)
- **`ragnar-core`** — glue only: launchers, labwc bridge, schemas,
  icons, data. No compiled code, no legacy deps
- **All desktop components Rust**: taskbar, view, settings,
  run-dialog, power-dialog, screenshot, polkit-agent, raven,
  automount, display-svc, brightness-helper, screenlock, wallpaper,
  osd, mpris, keyboard-layout, status-notifier, nightlight,
  gtk3-sync, keyring
- **`ragnar-settings` 1.0.12** — full GUI coverage including
  keybinds, printers with PDF output, and complete de/en
  localisation
- **System cleaned**: legacy packages purged, dead autostarts/schemas
  removed, source trees cleaned and synced
- **labwc `autoEnableOutputs`** — fixed the black-screen live boot:
  the compositor now enables outputs on a fresh session instead of
  waiting for a saved display profile

## Open

- `ragnar-session`: evaluate `EndSessionResponse` (logged only),
  cargo test suite, port `compositor-ready` + `session-quit`/`-inhibit`
  to Rust (last C leftovers)
- `ragnar-settings`: panel position/autohide, terminal selection
  (if schema extended), OSK/screen reader
- `ragnar-keyring`: test behaviour on fresh installs,
  ssh-agent component, password change for existing keyrings
- **Odin**: own compositor — in development, replaces labwc once
  stable (see README "Compositor")
- `ragnar-polkit-agent`: investigate recurring "OBJECT EXPORT
  FAILED" journal spam
- General: screenshots for the README, integration tests,
  `cargo test` for all crates
