# ragnar-taskbar

Wayland-native, GTK4-based taskbar for the Ragnar desktop (LieraOS).
Written in Rust; rendered through `wlr-layer-shell` via gtk4-layer-shell.

## Features

- Classic taskbar layout: start menu left, task list centre,
  tray + clock right — as a freely ordered applet sequence with
  flexible or fixed-width spacers (`menu;spacer;tasklist;spacer;tray;status;clock`)
  edited as a live miniature of the bar: gaps render as dashed
  stretchy segments, "+" slots insert gaps in place, chips reorder
  via drag & drop, one click resets the arrangement
- Optional animated gradient background per bar (off by default):
  user-picked colour list, flow direction (→/←/↓/↑/spinning), speed
  and a 15/30/60 fps smoothness preset — painted as a Cairo gradient
  layer under the applets, zero overhead while disabled
- Task list: real running windows via `zwlr_foreign_toplevel_manager_v1`
  (activate/minimize on left-click; right-click → minimize /
  maximize / close)
- Workspace switcher: real workspaces via `ext-workspace-v1`
  (number buttons, active highlighted, click → activate); the
  settings page sets both the compositor's workspace count
  (writes labwc's `<desktops number>` + live reconfigure) and a
  per-bar cap on shown buttons
- System tray: StatusNotifier host against ragnar-daemon's watcher —
  real items incl. ayatana paths and IconThemePath icons (Discord,
  Steam, Blueman, Heimdall); right-click → the app's own
  `com.canonical.dbusmenu` menu with real cascading submenus, item
  icons (icon-name + icon-data), check/radio toggles, separators
  and live layout updates via LayoutUpdated
- Clock: left-click → Cinnamon-style calendar panel (date, live
  clock, month calendar, "Datums- und Zeiteinstellungen");
  right-click → "Einrichten …"; appointment days are marked
  straight from Raven's appointments.ics and double-click or
  right-click on a day opens the Raven editor
- Task-button hover → live window thumbnail preview via
  ext-image-copy-capture (title + scaled texture over the applet)
- Multi-bar: bars are spawned/managed at runtime from a registry;
  the settings window edits every bar independently (edge incl.
  vertical left/right, geometry, applets, opacity, colour)
- Categorised start menu (hover-switchable GtkStack, search,
  session actions, Ragnar logo) — categories and per-app
  assignments are user-editable via menu.ini and the settings page
  (drag & drop onto categories, per-app rename/hide, hover vs click
  switching, custom categories)
- Hardware-sensor applets (CPU/GPU/RAM/motherboard temps) read
  sysfs hwmon directly — nvidia-smi fallback for proprietary GPUs;
  custom `cmd:` applets run user shell commands and show stdout
- Menus are single fullscreen layer-shell overlay surfaces —
  outside click or ESC dismisses, no input grabs, positioned above
  the clicked applet
- Right-click on empty bar space → "Eigenschaften" opens the
  settings window: bars on four edges incl. vertical, height,
  width, alignment, icon size, opacity + colour per bar,
  drag-and-drop applet ordering, per-bar applet checkboxes,
  custom cmd: applets; the bar column is a draggable GtkPaned
  divider, and the start-menu page collapses it to a slim
  back strip so the menu manager gets the full width; the detail
  page scrolls vertically, and the wheel over a slider scrolls the
  page instead of changing the value
- Notification bell: pure reader of ragnar-raven's shared
  history.jsonl — visible only while unread entries exist, opening
  the list marks them seen
- Per-bar config in `~/.config/ragnar-taskbar/bars.ini` — one
  `[bar-N]` section per bar; built for multiple bars
- The standalone panel of the Ragnar desktop

## i18n

UI is gettext-based: German is the msgid language, `po/en.po`
provides English (`/usr/share/locale/…/ragnar-taskbar.mo`).
`cargo test` covers the pure parsers (rc.xml desktops, cmd:
specs, workspace label shortening).

## Build

```sh
sudo apt install libgtk4-layer-shell-dev
cargo build --release
./target/release/ragnar-taskbar
```

Runtime dependency: `libgtk4-layer-shell0`.
`wayland-client` uses runtime loading (`dlopen`) so libwayland never
precedes gtk4-layer-shell in link order — that order would silently
break layer-shell support.
