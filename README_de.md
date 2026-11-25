# Ragnar — ein modulares Wayland-Desktop-Environment

Ragnar ist ein modulares Desktop Environment für Wayland, gebaut auf
Standard-Linux- und freedesktop-Mechanismen: systemd, D-Bus, polkit,
XDG, PipeWire. Alle Komponenten sind eigenständige Rust/GTK4-Krates:
jede einzeln baubar, paketierbar und austauschbar.

English version: [README.md](README.md)

## Compositor

Ragnar läuft aktuell auf **labwc**, angebunden über die labwc-Bridge
in `ragnar-desktop`. Der eigene Compositor **Odin** — ein
eigenständiger Wayland-Compositor in Rust (kein wlroots, kein
Smithay; der gesamte Stack von DRM/KMS über einen Vulkan-Renderer
bis zum eigenen XWayland-Fenstermanager ist im eigenen Baum
implementiert) — ist in Arbeit und ersetzt labwc, sobald er stabil
ist. Da alle Komponenten ausschließlich Standard-Wayland-Protokolle
und die eigenen D-Bus-APIs verwenden, ist der Wechsel ein
Single-Point-Change — keine Komponente muss dafür umgeschrieben
werden.

## Komponenten

| Komponente | Zweck |
| --- | --- |
| `ragnar-taskbar` | Panel, Startmenü, Tray, Lautstärke-Applet |
| `ragnar-view` | Desktop-Icons |
| `ragnar-settings` | Settings-Zentrale |
| `ragnar-skald` | Audio-Frontend + Daemon (PipeWire/WirePlumber) |
| `ragnar-raven` | Benachrichtigungen, Seitenleiste, Kalender (CalDAV) |
| `ragnar-heimdall` | Netzwerk/Verbindungen (iwd, systemd-networkd) |
| `ragnar-software` | Softwareverwaltung (PackageKit + Flatpak) |
| `ragnar-taskmanager` | Task-Manager (procfs, systemd-D-Bus) |
| `ragnar-edit` | Texteditor |
| `ragnar-eye` | Bildbetrachter |
| `ragnar-calc` | Taschenrechner |
| `ragnar-clipboard` | Zwischenablage-Daemon + Verlaufs-Picker |
| `ragnar-sap` | Scannen und Drucken (SANE/CUPS) |
| `valkyrie` | Dateimanager (eigenes Repository) |
| `ragnarok` | Systemfehler-Übersicht |
| `ragnar-session` | Session-Orchestrierung |
| `ragnar-desktop` | Session-Glue: Launcher, Schemas, Daten (Meta-Paket) |
| `ragnar-run-dialog` | Alt+F2-Launcher |
| `ragnar-power-dialog` | Logout/Neustart/Herunterfahren |
| `ragnar-screenlock` | Sperrbildschirm |
| `ragnar-polkit-agent` | polkit-Authentifizierungs-Agent |
| `ragnar-screenshot` | Screenshot-Werkzeug |
| `ragnar-osd` | On-Screen-Display (Lautstärke, Helligkeit) |
| `ragnar-nightlight` | Nachtlicht/Farbtemperatur |
| `ragnar-status-notifier` | System-Tray (StatusNotifier) |
| `ragnar-mpris` | MPRIS-Mediensteuerung |
| `ragnar-automount` | Automount für Wechselmedien |
| `ragnar-brightness-helper` | Backlight via logind (Helligkeits-Tasten) |
| `ragnar-display-svc` | Display-Verwaltungsdienst |
| `ragnar-gtk3-sync` | GTK3↔GTK4 Theme-/Einstellungs-Sync |
| `ragnar-keyboard-layout` | Tastatur-Layouts |
| `ragnar-keyring` | Session-Keyring (Secret-Service-Start) |
| `ragnar-wallpaper` | Hintergrundbild-Verwaltung |
| `ragnar-shared` | Gemeinsame Rust-Bibliothek (D-Bus, Display, GPU, Config) |
| `liera-secrets` | Credential-Store (freedesktop Secret Service) |

## Layout

```
ragnar-<name>/
├── src/            # Rust-Quellen
├── po/             # Übersetzungen (.po — kompilierte .mo sind Build-Artefakte)
├── data/           # Icons, Schemas, Desktop-Dateien
├── pkg/            # Deb-Staging-Skeleton (DEBIAN/, Services, Configs —
│                   #   kompilierte Binaries werden vom Build hineinkopiert
│                   #   und sind NICHT im Repo)
├── packaging/      # build-deb.sh (falls vorhanden)
├── tests/
├── README.md       # zweisprachig / bilingual
└── TODO.md         # zweisprachig / bilingual
```

## Build

Jede Komponente baut eigenständig und erzeugt ein `.deb`:

```bash
cd ragnar-<name>
cargo fmt -- --check
cargo clippy
cargo test
cargo build --release
packaging/build-deb.sh        # oder: dpkg-deb --build pkg/<name>
```

## Konventionen

- **Code:** Englisch, Rust bevorzugt. Kommentare erklären Module/Funktionen.
- **Doku:** README + TODO zweisprachig (Deutsch/Englisch), permanent gepflegt.
- **Repo:** Nur Quellcode. Keine Binaries, `.deb`s, `target/` oder
  kompilierte `.mo` (siehe `.gitignore`).
- **Architektur:** Komponenten kommunizieren über Standardschnittstellen
  (Wayland-Protokolle, D-Bus) — nie über compositorspezifische Interna,
  damit jede Komponente einzeln ersetzbar bleibt.

## Screenshots

<!--
  PNG-Screenshots des laufenden Desktops in docs/screenshots/ ablegen
  und Dateinamen + Alt-Texte unten anpassen. Vorschläge: voller
  Desktop, Ragnar Settings, Heimdall, Valkyrie oder eine App-/Gaming-
  Ansicht. Das 2x2-Tabellenlayout beibehalten — GitHub rendert
  Bild-Tabellen sauber.
-->

| Desktop | Ragnar Settings |
| --- | --- |
| ![Ragnar-Desktop mit Taskbar](docs/screenshots/desktop.png) | ![Ragnar Settings](docs/screenshots/settings.png) |

| Heimdall | Apps |
| --- | --- |
| ![Ragnar Heimdall Netzwerk-Manager](docs/screenshots/heimdall.png) | ![Anwendungen auf dem Ragnar-Desktop](docs/screenshots/apps.png) |

## Lizenz

GPL-3.0-or-later (siehe LICENSE).
