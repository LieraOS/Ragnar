# Ragnar — a modular Wayland desktop environment

Ragnar is a modular Wayland desktop environment built on standard
Linux and freedesktop mechanisms: systemd, D-Bus, polkit, XDG,
PipeWire. Every component is an independent Rust/GTK4 crate: each one
buildable, packageable and replaceable on its own.

Deutsche Version: [README_de.md](README_de.md)

## Compositor

Ragnar currently runs on **labwc**, integrated through the labwc
bridge in `ragnar-desktop`. Our own compositor **Odin** — a
standalone Wayland compositor written in Rust (no wlroots, no
Smithay; the whole stack from DRM/KMS over a Vulkan renderer to
its own XWayland window manager is implemented in-tree) — is in
development and will replace labwc once it is stable. Since every
component only uses standard Wayland protocols and our own D-Bus
APIs, the swap is a single-point change — no component has to be
rewritten for it.

## Components

| Component | Purpose |
| --- | --- |
| `ragnar-taskbar` | Panel, start menu, tray, volume applet |
| `ragnar-view` | Desktop icons |
| `ragnar-settings` | Settings center |
| `ragnar-skald` | Audio frontend + daemon (PipeWire/WirePlumber) |
| `ragnar-raven` | Notifications, sidebar, calendar (CalDAV) |
| `ragnar-heimdall` | Network/connections (iwd, systemd-networkd) |
| `ragnar-session` | Session orchestration |
| `ragnar-desktop` | Session glue: launchers, schemas, data (meta-package) |
| `ragnar-run-dialog` | Alt+F2 launcher |
| `ragnar-power-dialog` | Logout/reboot/shutdown |
| `ragnar-screenlock` | Lock screen |
| `ragnar-polkit-agent` | polkit authentication agent |
| `ragnar-screenshot` | Screenshot tool |
| `ragnar-osd` | On-screen display (volume, brightness) |
| `ragnar-nightlight` | Night light/color temperature |
| `ragnar-status-notifier` | System tray (StatusNotifier) |
| `ragnar-mpris` | MPRIS media control |
| `ragnar-automount` | Automount for removable media |
| `ragnar-brightness-helper` | Backlight via logind (brightness keys) |
| `ragnar-display-svc` | Display management service |
| `ragnar-gtk3-sync` | GTK3↔GTK4 theme/settings sync |
| `ragnar-keyboard-layout` | Keyboard layouts |
| `ragnar-keyring` | Session keyring (Secret Service start) |
| `ragnar-wallpaper` | Wallpaper management |
| `ragnar-shared` | Shared Rust library (D-Bus, display, GPU, config) |
| `liera-secrets` | Credential store (freedesktop Secret Service) |

## Layout

```
ragnar-<name>/
├── src/            # Rust sources
├── po/             # translations (.po — compiled .mo files are build artifacts)
├── data/           # icons, schemas, desktop files
├── pkg/            # deb staging skeleton (DEBIAN/, services, configs —
│                   #   compiled binaries are copied in by the build
│                   #   and are NOT in the repo)
├── packaging/      # build-deb.sh (where present)
├── tests/
├── README.md       # bilingual / zweisprachig
└── TODO.md         # bilingual / zweisprachig
```

## Build

Every component builds on its own and produces a `.deb`:

```bash
cd ragnar-<name>
cargo fmt -- --check
cargo clippy
cargo test
cargo build --release
packaging/build-deb.sh        # or: dpkg-deb --build pkg/<name>
```

## Conventions

- **Code:** English, Rust preferred. Comments explain modules/functions.
- **Docs:** README + TODO kept in German and English, always current.
- **Repo:** source code only. No binaries, `.deb`s, `target/` or
  compiled `.mo` files (see `.gitignore`).
- **Architecture:** components communicate through standard
  interfaces (Wayland protocols, D-Bus) — never through
  compositor-specific internals, so every component stays
  individually replaceable.

## Screenshots

<!--
  Drop PNG screenshots into docs/screenshots/ and adjust the file
  names + alt text below. Suggested shots: full desktop, Ragnar
  Settings, Heimdall, Valkyrie or a gaming/app view.
  Keep the 2x2 table layout; GitHub renders image tables nicely.
-->

| Desktop | Ragnar Settings |
| --- | --- |
| ![Ragnar desktop with taskbar](docs/screenshots/desktop.png) | ![Ragnar Settings](docs/screenshots/settings.png) |

| Heimdall | Apps |
| --- | --- |
| ![Ragnar Heimdall network manager](docs/screenshots/heimdall.png) | ![Applications on the Ragnar desktop](docs/screenshots/apps.png) |

## License

GPL-3.0-or-later (see LICENSE).
