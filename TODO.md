# Ragnar TODO

## Deutsch

Ziel erreicht: Ragnar läuft vollständig frei von Fork-Altlasten —
alle Komponenten sind eigenständige Rust-Crates oder Skripte, einzeln
austauschbar, neutrale Schnittstellen (Wayland-Protokolle, D-Bus).

### Erledigt (verifiziert auf dem produktiven System)

- **`ragnar-session` 2.x (Rust)** — ersetzt den Session-Manager
  komplett: `org.gnome.SessionManager`-API, RegisterClient/
  ClientPrivate-Handshake, RequiredComponents, EndSessionDialog-
  Orchestrierung, Inhibit-API, `ragnar-session.target` aktiviert den
  generierten Autostart-Baum (Stop+Start pro Login)
- **`ragnar-core`** — nur noch Glue: Launcher, labwc-Bridge,
  Schemas, Icons, Daten. Kein kompilierter Code, keine Alt-Deps
- **Alle Desktop-Komponenten Rust**: taskbar, view, settings,
  run-dialog, power-dialog, screenshot, polkit-agent, raven,
  automount, display-svc, brightness-helper, screenlock, wallpaper,
  osd, mpris, keyboard-layout, status-notifier, nightlight,
  gtk3-sync, keyring
- **System gesäubert**: Alt-Pakete gepurged, tote Autostarts/Schemas
  entfernt, Source-Bäume bereinigt und synchron

### Offen

- `ragnar-session`: `EndSessionResponse` auswerten (derzeit geloggt),
  cargo-Test-Suite, `compositor-ready` + `session-quit`/`-inhibit`
  in Rust portieren (letzte C-Reste)
- `ragnar-settings`: Übersetzungen (po/ leer), Panel-Position/
  Autohide, Software/Printers-Panels, Software-Center-Tile,
  Terminal-Auswahl (falls Schema erweitert), OSK/Screenreader
- `ragnar-keyring`: Verhalten auf frischen Installationen testen,
  ssh-Agent-Komponente, Passwort-Änderung bestehender Bünde
- `valkyrie`: Tiefenanalyse des Dateimanagers (GitHub-Baum enthält
  bisher nur README — Quellbaum liegt lokal)
- Allgemein: README-Dateien final schreiben (Benutzer),
  Integrations-Tests, `cargo test` für alle Crates

## English

Goal reached: Ragnar runs completely free of fork legacy — every
component is a standalone Rust crate or script, individually
replaceable, using neutral interfaces (Wayland protocols, D-Bus).

### Done (verified on the production system)

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
- **System cleaned**: legacy packages purged, dead autostarts/schemas
  removed, source trees cleaned and synced

### Open

- `ragnar-session`: evaluate `EndSessionResponse` (logged only),
  cargo test suite, port `compositor-ready` + `session-quit`/`-inhibit`
  to Rust (last C leftovers)
- `ragnar-settings`: translations (po/ empty), panel position/
  autohide, Software/Printers panels, Software-Center tile,
  terminal selection (if schema extended), OSK/screen reader
- `ragnar-keyring`: test behaviour on fresh installs,
  ssh-agent component, password change for existing keyrings
- `valkyrie`: deep file manager analysis (GitHub tree holds only
  a README — the source tree lives locally)
- General: final READMEs (user), integration tests, `cargo test`
  for all crates
