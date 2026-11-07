# Ragnar — a modular Wayland desktop environment

**Deutsch:** Ragnar ist ein modulares Desktop Environment für Wayland,
gebaut auf Standard-Linux- und freedesktop-Mechanismen: systemd, D-Bus,
polkit, XDG, PipeWire. Alle Komponenten sind eigenständige Rust/GTK4-Krates:
jede einzeln baubar, paketierbar und austauschbar.

**English:** Ragnar is a modular Wayland desktop environment built on
standard Linux and freedesktop mechanisms: systemd, D-Bus, polkit, XDG,
PipeWire. Every component is an independent Rust/GTK4 crate: each one
buildable, packageable and replaceable on its own.

## Components / Komponenten

| Component | Zweck / Purpose |
| --- | --- |
| `ragnar-taskbar` | Panel, Startmenü, Tray, Lautstärke-Applet |
| `ragnar-view` | Desktop-Icons |
| `ragnar-settings` | Settings-Zentrale |
| `ragnar-skald` | Audio-Frontend + Daemon (PipeWire/WirePlumber) |
| `ragnar-raven` | Benachrichtigungen, Seitenleiste, Kalender (CalDAV) |
| `ragnar-heimdall` | Netzwerk/Verbindungen (iwd, systemd-networkd) |
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

**Deutsch:** Jede Komponente baut eigenständig und erzeugt ein `.deb`:

**English:** Every component builds on its own and produces a `.deb`:

```bash
cd ragnar-<name>
cargo fmt -- --check
cargo clippy
cargo test
cargo build --release
packaging/build-deb.sh        # oder / or: dpkg-deb --build pkg/<name>
```

## Konventionen / Conventions

- **Code:** Englisch, Rust bevorzugt. Kommentare erklären Module/Funktionen.
- **Doku:** README + TODO zweisprachig (Deutsch/Englisch), permanent gepflegt.
- **Repo:** Nur Quellcode. Keine Binaries, `.deb`s, `target/` oder
  kompilierte `.mo` (siehe `.gitignore`).
- **Architektur:** Komponenten kommunizieren über Standardschnittstellen
  (Wayland-Protokolle, D-Bus) — nie über compositorspezifische Interna,
  damit jede Komponente einzeln ersetzbar bleibt.

## License

GPL-3.0-or-later (siehe LICENSE / see LICENSE).
