# ragnar-livewall

Animated live wallpaper for the Ragnar desktop (Liera).

A small `zwlr_layer_shell` client that puts a wallpaper picture on
the compositor's **background layer** — the same slot `swaybg`
occupies — and draws slow procedural aurora veils across the sky
region. Because it uses only standard Wayland protocols it runs on
any layer-shell compositor: labwc today, Odin later.

## Features

- One background surface per monitor (multi-monitor aware)
- Procedural aurora rendered in an OpenGL fragment shader
  (fbm-noise curtains with drifting ray striations) on a
  GtkGLArea — falls back to the static picture if GL init
  fails. 15 fps is enough: a real aurora crawls, so the GPU
  cost stays negligible
- **Automatic pause**: when any toplevel window goes fullscreen
  (games, video) the animation freezes to zero GPU cost, detected
  via `zwlr_foreign_toplevel_v1` on a second Wayland connection
- Cabin lamp flicker on the stock artwork: lit windows
  shimmer, occasionally burst-flicker or briefly go out
- **Configurable effects** (glow, stars, mist, snow, rain,
  fireflies, cloud shadows) per wallpaper via
  `~/.config/ragnar-settings/livewall-effects.ini` — each is a
  toggleable shader block; all but the single-band glow accept up
  to four regions. Regions and parameters are shader uniforms; the
  effects editor in
  ragnar-settings writes the file and its GL preview runs the
  very GLSL installed under `/usr/share/ragnar-livewall/`
- No keyboard focus, no window frame — pure wallpaper

## Usage

```sh
ragnar-livewall                # stock wallpaper
ragnar-livewall --image P.png  # animate a different picture
```

Run it from a terminal; `Ctrl-C` stops it. On a Ragnar session the
`ragnar-wallpaper` daemon spawns it automatically (one process for
all monitors) while the *Animated wallpaper* switch in
ragnar-settings is on — the app registers no D-Bus name
(`ApplicationFlags::NON_UNIQUE`) so spawned children never exit
early.

## Building

```sh
cargo build --release
```

Runtime dependencies: GTK 4.16+, a compositor with
`zwlr_layer_shell_v1` (v4) and optionally
`zwlr_foreign_toplevel_manager_v1` for the auto-pause.

## Planned

- Pause on lock screen / battery
- Per-wallpaper effect sidecar config + custom animation editor
  with live preview in ragnar-settings
- Live-build integration once approved

## License

GPL-3.0-or-later. Copyright (C) Jörg Schröder.
