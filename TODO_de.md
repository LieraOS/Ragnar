# Ragnar TODO

English version: [TODO.md](TODO.md)

Ziel erreicht: Ragnar läuft vollständig frei von Fork-Altlasten —
alle Komponenten sind eigenständige Rust-Crates oder Skripte, einzeln
austauschbar, neutrale Schnittstellen (Wayland-Protokolle, D-Bus).

## Erledigt (verifiziert auf dem produktiven System)

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
- **`ragnar-settings` 1.0.12** — volle GUI-Abdeckung inklusive
  Keybinds, Drucker mit PDF-Ausgabe und kompletter de/en-
  Lokalisierung
- **System gesäubert**: Alt-Pakete gepurged, tote Autostarts/Schemas
  entfernt, Source-Bäume bereinigt und synchron
- **labwc `autoEnableOutputs`** — Schwarzer-Bildschirm-Bug beim
  Live-Boot behoben: Der Compositor aktiviert Outputs jetzt direkt
  beim Session-Start, statt auf ein gespeichertes Display-Profil
  zu warten

## Offen

- `ragnar-session`: `EndSessionResponse` auswerten (derzeit geloggt),
  cargo-Test-Suite, `compositor-ready` + `session-quit`/`-inhibit`
  in Rust portieren (letzte C-Reste)
- `ragnar-settings`: Panel-Position/Autohide, Terminal-Auswahl
  (falls Schema erweitert), OSK/Screenreader
- `ragnar-keyring`: Verhalten auf frischen Installationen testen,
  ssh-Agent-Komponente, Passwort-Änderung bestehender Bünde
- **Odin**: eigener Compositor — in Arbeit, ersetzt labwc sobald
  stabil (siehe README „Compositor")
- `ragnar-polkit-agent`: wiederkehrendes „OBJECT EXPORT FAILED"-
  Journal-Spam untersuchen
- Allgemein: Screenshots für die README, Integrations-Tests,
  `cargo test` für alle Crates
