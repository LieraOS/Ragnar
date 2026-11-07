# Ragnar Settings — the LieraOS control hub

Ragnar Settings is the native control center for the Ragnar desktop,
written in **Rust + GTK4**. The goal is *familiarity, not imitation*: the
layout should feel immediately obvious to people coming from Windows —
a categorized overview where every area is clearly labeled and reachable
in one click — while remaining a native LieraOS application with its own
identity. Each area is either an embedded native panel or launches an
existing LieraOS application.

## Status

Skeleton implemented and verified: the app builds and runs, shows the
categorized hub with search, and opens the first real panel
("About this system"). All catalog plumbing (registry, availability
checks, external launcher) works; remaining catalog entries show a
placeholder until their real panel lands. Until Ragnar Settings is
feature-complete, the `ragnar-control-center` package
remains installed; both coexist. Only when Ragnar Settings covers
everything needed does `ragnar-control-center` get dropped from the ISO
package list.

## Architecture

- **Hub window** — `adw::NavigationView`; the root page is a categorized
  tile grid with a search field. Activating a tile pushes the panel's
  navigation page (title and back button come from the navigation view).
- **Registry catalog** (`src/registry/`) — every area is one `Entry`
  (id, title, subtitle, icon, category, action, requirement, keywords).
  A tile either embeds a native panel (`Action::Panel`) or launches an
  external program (`Action::Launch`). Adding an area means adding an
  entry — no hub changes needed.
- **Availability checks** (`src/backend/`) — entries declare what they
  need (`Always`, `Program`, `Schema`); unsatisfiable tiles are hidden,
  so the same binary runs on a development machine without Ragnar and
  shows everything inside a Ragnar session.
- **Native panels** (`src/panels/`) for Ragnar-specific settings:
  - *Display / Personalization* — wallpaper, icon theme/settings,
    resolution (wlr-output-management under labwc)
  - *Sounds* — install and select sound themes
  - *Panel & Desktop* — applets, Raven, autostart, default apps
  - *Removable media* — automount switch
    (`org.lieraos.ragnar.automount enabled`, consumed live by
    ragnar-automount) plus a live list of mounted removable volumes
    with unmount/eject buttons (gio::VolumeMonitor)
- **Launched apps** — tiles that open existing LieraOS tools:
  - *Sound* → **LieraSound** (volume, devices, effects)
  - *Network & Bluetooth* → **Heimdall** (in development)
  - *Software* → installed-apps overview → **Liera Software Center**

## Development

```sh
cargo build        # build
cargo test         # unit tests (registry search, ...)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run          # run locally
```

## Development notes

- GTK 4.18 crashes with a stack overflow in `real_choose_icon` when the
  icon-theme fallback chain is broken (e.g. a partial local
  `~/.local/share/icons/hicolor/index.theme` shadowing the system
  theme). A bare `GtkHeaderBar` reproduces it system-wide. If the app
  dies instantly on a dev machine, check the local icon themes first —
  this is a host defect, not an app bug.

## Recent changes / Letzte Änderungen

- **EN:** "Removable media" panel added: automount switch bound to
  `org.lieraos.ragnar.automount enabled` (schema shipped by this
  package and compiled via `postinst`), and a live "Connected
  devices" section listing mounted removable volumes with
  unmount/eject buttons.
- **DE:** Panel „Wechseldatenträger" hinzugefügt: Automount-Schalter
  an `org.lieraos.ragnar.automount enabled` gebunden (Schema wird von
  diesem Paket ausgeliefert und per `postinst` kompiliert) sowie eine
  Live-Sektion „Verbundene Geräte" mit gemounteten Wechseldatenträgern
  und Unmount-/Eject-Buttons.
- **EN:** Three more panels landed:
  - *Night light* — gammastep-backed (the only gamma consumer under
    labwc); state lives in `~/.config/gammastep/config.ini` +
    a managed autostart entry. No "sunset to sunrise" mode because
    no location service (GeoClue) is installed.
  - *Updates* — pending count from `apt-get -s dist-upgrade`, pkexec
    refresh/install, and an auto-upgrade switch writing
    `/etc/apt/apt.conf.d/52ragnar-auto-upgrades`. Liera-specific:
    `lsb_release -cs` prints `n/a`, so the shipped
    `${distro_codename}` origins patterns would match nothing — the
    drop-in pins `origin=Debian,label=Debian` instead.
  - *Accessibility* — text-scaling-factor, enable-animations
    (inverted as "Reduce animations"), high-contrast; all guarded by
    schema lookups. Shared helpers moved into
    `src/panels/common.rs` + a generic INI module `src/panels/ini.rs`.
- **DE:** Drei weitere Panels umgesetzt:
  - *Nachtlicht* — über gammastep; Zustand in
    `~/.config/gammastep/config.ini` + verwalteter Autostart-Datei.
    Kein „Sonnenuntergang bis Sonnenaufgang"-Modus, da kein
    Ortungsdienst (GeoClue) installiert ist.
  - *Aktualisierungen* — Anzahl ausstehender Updates via
    `apt-get -s dist-upgrade`, pkexec-Auffrischung/-Installation und
    ein Schalter für automatische Updates, der
    `/etc/apt/apt.conf.d/52ragnar-auto-upgrades` schreibt.
    Liera-spezifisch: `lsb_release -cs` liefert `n/a`, daher würden
    die mitgelieferten `${distro_codename}`-Muster nichts treffen —
    das Drop-in pinnt stattdessen `origin=Debian,label=Debian`.
  - *Barrierefreiheit* — Textskalierung, Animationen reduzieren,
    hoher Kontrast; alle per Schema-Lookup abgesichert. Gemeinsame
    Helfer nach `src/panels/common.rs` und generisches INI-Modul
    `src/panels/ini.rs` ausgelagert.

## Packaging (later)

Will ship as its own Debian binary package `ragnar-settings`, built
from this directory (native component — no upstream import, no quilt
rename needed). It joins `build-env/out/extracted/` only when it is
ready for ISO builds.
