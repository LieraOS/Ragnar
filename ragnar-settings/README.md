# Ragnar Settings — the LieraOS control hub

Ragnar Settings is the native control center for the Ragnar desktop,
written in **Rust + GTK4**. The goal is *familiarity, not imitation*: the
layout should feel immediately obvious to people coming from Windows —
a categorized overview where every area is clearly labeled and reachable
in one click — while remaining a native LieraOS application with its own
identity. Each area is either an embedded native panel or launches an
existing LieraOS application.

## Status

Fully working control center: categorized hub with live search,
~20 embedded panels (display, appearance, sounds, updates, users,
default applications, removable media, night light, accessibility, …)
plus tiles that launch other LieraOS tools. Two audience levels:

- **Normal view** (default) — everything a typical desktop user
  expects: display, wallpaper, language, panel & desktop, date/time,
  default applications, updates, drivers.
- **Advanced mode** — a persistent switch in the header bar next to
  the search field (*Advanced settings* / *Erweiterte
  Einstellungen*). Enabling it exposes depth that would otherwise
  require editing configuration files: the GPU controls inside
  Display (power limit, locked clock, fan speed), low-level input
  configuration, group management in Users, and any future tiles
  registered in `registry::catalog::ADVANCED_TILES`.

The hub footer holds **Back up / Restore configuration**: export
packs every configuration source (config files, gsettings dumps,
system files) into one timestamped `.tar.gz` chosen by the user;
restore unpacks it and replays everything (system files via one
pkexec call). The full source list lives in `src/backup/mod.rs` as a
declarative manifest — extending the app means extending that list.

Advanced state persists in `~/.config/ragnar-settings/settings.ini`
(`[ui] advanced=true|false`); tiles and guarded panel sections
evaluate it every time they are built, so toggling takes effect on
the next hub rebuild / panel open.

## Architecture

- **Hub window** — `GtkStack` + `ToolbarView`; the hub page is a
  categorized tile grid with a search field and the advanced-mode
  switch in the header, plus backup/restore buttons in a fixed
  footer. Tiles push panel pages onto the stack; a back button
  returns to the hub.
- **Registry catalog** (`src/registry/`) — every area is one `Entry`
  (id, title, subtitle, icon, category, action, requirement, keywords).
  `catalog.rs` holds the types + accessors, `entries.rs` the tile list.
  A tile either embeds a native panel (`Action::Panel`) or launches an
  external program (`Action::Launch`). Adding an area means adding an
  entry — no hub changes needed.
- **Availability checks** (`src/backend/`) — entries declare what they
  need (`Always`, `Program`, `Schema`); unsatisfiable tiles are hidden,
  so the same binary runs on a development machine without Ragnar and
  shows everything inside a Ragnar session.
- **Compositor capabilities** (`src/backend/compositor.rs`) — detects
  the running compositor (labwc today, the native Ragnar compositor
  later) and maps what it can do. Deep settings rows declare the
  capability they need: supported controls stay editable, everything
  else is shown locked with "Requires the Ragnar compositor." — and
  unlocks by itself once the native compositor runs. The map is the
  only place that ever needs updating.
- **rc.xml patcher** (`src/panels/rcxml.rs`) — text-level editor for
  the compositor configuration file. It replaces existing elements in
  place, appends missing keys inside existing sections, creates
  missing top-level sections before `</labwc_config>` and writes
  attributes (including on attribute-keyed elements like
  `<font place="…">`). User-managed content (keybinds, window rules,
  comments) is never touched; `labwc -r` applies edits live.
- **Native panels** (`src/panels/`) for Ragnar-specific settings:
  - *Display / Personalization* — wallpaper, icon theme/settings,
    resolution (wlr-output-management under labwc), GPU controls
    (advanced)
  - *Sounds* — install and select sound themes, per-event assignment
  - *Panel & Desktop* — applets, Raven, autostart, default apps
  - *Removable media* — automount switch
    (`org.lieraos.ragnar.automount enabled`, consumed live by
    ragnar-automount) plus a live list of mounted removable volumes
    with unmount/eject buttons (gio::VolumeMonitor)
  - *Compositor* (advanced) — window decoration (corner radius, drop
    shadows, keep-border, titlebar layout/font), window gap and
    work-area margins, workspace count + popup time, focus behaviour
    (follow-mouse, raise, delay), snap distances, top-maximize,
    new-window placement and tearing — all written live to the
    compositor configuration. Includes per-workspace names shown
    in the switcher (positional <desktops><names> entries). A "Coming with the native compositor"
    section lists locked features (blur, animations, tiling layouts,
    gestures, window groups, scratchpad, mirroring) that unlock
    automatically once the compositor provides them.
  - *Window rules* (advanced, experimental) — full editor for the
    compositor's window rules: match by app id or title (globs),
    then per-rule window state (always on top/below, all
    workspaces, maximized/fullscreen/minimized), placement
    (snap edge, center, target workspace) and behaviour
    (titlebar override, taskbar/switcher hiding, focus block,
    locked position). Attributes and actions the UI does not
    model are preserved verbatim; rules save live.
  - *Keyboard shortcuts* — full keybind editor: capture a combo and
    bind it to any compositor action — run a command, close/kill/
    minimize/maximize/fullscreen windows, snap to edges, go to or
    send windows to workspaces, window switching, menus, show
    desktop, reload. Session-managed binds show read-only.
  - *Autostart & environment* (advanced) — session autostart file
    (programs at login) and environment variables, edited in place
    with comments preserved; session-managed variables (keyboard,
    cursor, locale) show read-only.
- **Launched apps** — tiles that open existing LieraOS tools:
  - *Sound* → **LieraSound** (volume, devices, effects)
  - *Network & Bluetooth* → **Heimdall** (in development)
  - *Software* → installed-apps overview → **Liera Software Center**

## Translations

English is the source language; all user-visible strings go through
`gettext()` (or the `msgid()` marker for strings translated later at
display time). A complete German catalog ships in `po/de.po`;
`build.rs` compiles every `po/*.po` with `msgfmt` into
`target/locale/<lang>/LC_MESSAGES/` and the Debian package installs
them under `/usr/share/locale/`. `LOCALEDIR` overrides the lookup
path for development runs.

Update the catalogs after touching user-visible strings:

```sh
xgettext --language=Rust --keyword=gettext --keyword=msgid \
    --from-code=UTF-8 --package-name=ragnar-settings \
    --package-version=<ver> -o po/ragnar-settings.pot \
    src/*.rs src/*/*.rs src/*/*/*.rs
msgmerge -U po/de.po po/ragnar-settings.pot
```

## Development

```sh
cargo build        # build (also compiles po/*.po -> .mo)
cargo test         # unit tests (registry search, ini, apt, terminal)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run          # run locally
LANGUAGE=de LOCALEDIR=target/locale cargo run   # run in German
```

## Development notes

- GTK 4.18 crashes with a stack overflow in `real_choose_icon` when the
  icon-theme fallback chain is broken (e.g. a partial local
  `~/.local/share/icons/hicolor/index.theme` shadowing the system
  theme). A bare `GtkHeaderBar` reproduces it system-wide. If the app
  dies instantly on a dev machine, check the local icon themes first —
  this is a host defect, not an app bug.

## Recent changes / Letzte Änderungen

- **EN:** 1.0.12 — Compositor panel (advanced): window decoration,
  gaps/margins, workspaces, focus, snapping, placement and tearing
  written live to the compositor configuration through a generic
  rc.xml patcher (`src/panels/rcxml.rs`, replaces the input-only
  editor; input + shortcuts panels migrated). Locked rows for
  compositor-dependent features unlock automatically via the new
  capability map (`src/backend/compositor.rs`) when the native
  Ragnar compositor runs. Patcher covered by a fixture test.
  Also in 1.0.12: window-rules editor (advanced, experimental —
  matchers, state, placement, decoration; lossless round-trip) and
  the keyboard-shortcut editor v2 (full keybind editing with a
  WM-action catalogue; session-managed binds read-only), input
  depth (double-click time, tap-and-drag/drag lock, click & scroll
  methods, per-window layout scope) and a notification-position
  dropdown in the notifications panel. Also: the software tile
  launches ragnar-software, and the printers panel manages
  configured CUPS queues (default/remove, no add UI) plus the
  standard PDF printer — auto-provisioned via CUPS-PDF with
  editable paper size, resolution and PDF version defaults.
  Accessibility gained an experimental section with on-screen
  keyboard and screen-reader toggles writing the standard
  a11y.applications flags (each row reports whether a consumer
  like squeekboard or orca is installed; missing tools point to
  Software with an "Open Software" jump button). Fixed a
  hard crash in the Power panel (unguarded access to the
  gsd-power schema, which Ragnar does not ship); power profiles
  now use the net.hadess.PowerProfiles D-Bus API directly, so
  they work against power-profiles-daemon or tuned-ppd alike.
  Default Apps gained calendar and archive categories.
- **DE:** 1.0.12 — Compositor-Panel (erweitert): Fensterdekoration,
  Abstände/Ränder, Arbeitsflächen, Fokus, Snapping, Platzierung und
  Tearing — live in die Compositor-Konfiguration geschrieben durch
  einen generischen rc.xml-Patcher (`src/panels/rcxml.rs`, ersetzt
  den bisherigen Input-Editor; Input- + Shortcuts-Panel migriert).
  Gesperrte Zeilen für compositor-abhängige Funktionen werden über
  die neue Capability-Karte (`src/backend/compositor.rs`) automatisch
  freigeschaltet, sobald der native Ragnar-Compositor läuft. Patcher
  durch einen Fixture-Test abgedeckt.
- **EN:** 1.0.12 — Advanced settings mode (header-bar switch,
  persisted in `~/.config/ragnar-settings/settings.ini`);
  configuration backup & restore in the hub footer (single
  `.tar.gz`, manifest-driven in `src/backup/`); German catalog
  (`po/de.po`, ~510 strings) compiled by `build.rs` and shipped in
  the package; search field keeps focus while results rebuild (the
  pointer-grab workaround no longer runs on every keystroke);
  `display/canvas.rs` split into `monitor_rect.rs`, `catalog.rs` into
  `entries.rs`; all clippy warnings resolved; `eprintln!` replaced
  by `tracing` throughout.
- **DE:** 1.0.12 — Erweiterte Einstellungen (Schalter in der
  Kopfleiste, persistiert in
  `~/.config/ragnar-settings/settings.ini`); Konfigurations-
  Sicherung & Wiederherstellung im Hub-Footer (ein `.tar.gz`,
  manifestgesteuert in `src/backup/`); deutscher Katalog
  (`po/de.po`, ~510 Strings), von `build.rs` kompiliert und im
  Paket ausgeliefert; Suchfeld behält den Fokus während der
  Ergebnisaktualisierung; `display/canvas.rs` nach `monitor_rect.rs`
  und `catalog.rs` nach `entries.rs` aufgeteilt; alle
  Clippy-Warnungen behoben; `eprintln!` durchgängig durch `tracing`
  ersetzt.
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

## Packaging

Ships as the Debian binary package `ragnar-settings` (cargo-deb):
binary, `.desktop` file and compiled gettext catalogs under
`/usr/share/locale/<lang>/LC_MESSAGES/`.
