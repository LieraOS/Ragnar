# ragnar-wallpaper

Rust replacement for the background part of the legacy
`ragnar-daemon`. Standalone session service: reads
`org.gnome.desktop.background` (`picture-uri`, `picture-options`,
`primary-color`), keeps a `swaybg` process alive and respawns it on
every change — with a short hand-over so switches don't flicker.

**Modes:** GNOME `picture-options` → swaybg (`wallpaper`→`tile`,
`centered`→`center`, `scaled`→`fit`, `stretched`→`stretch`,
`zoom`/`spanned`→`fill`). Without an image or `options=none`:
`swaybg -m solid_color` using `primary-color`.

**Animated wallpaper (beta):** watches
`~/.config/ragnar-settings/wallpaper-animated` (`true`/`false`,
written by the ragnar-settings appearance panel). When the flag is
`true` and the choice is an image, a single `ragnar-livewall`
process replaces the per-output swaybg spawns — it owns the
background layer on every monitor. Flag off or spawn failure →
normal swaybg set. `ragnar-livewall` must run with
`ApplicationFlags::NON_UNIQUE`: GApplication single-instance
forwarding would make every spawned child exit immediately and
wedge the respawn logic. The same watcher also monitors
`livewall-effects.ini` (per-image effect parameters written by
the ragnar-settings effects editor) so edits respawn a running
livewall with the new values.

**Greeter sync:** writes the choice into AccountsService over the
system bus (`User.SetBackgroundFile` plus the DisplayManager
extension, LightDM convention) — the greeter shows the same image.
Best-effort, never fatal.

**Startup:** XDG autostart (`org.lieraos.ragnar.Wallpaper.desktop`).
The legacy daemon no longer does this job as of `ragnar-core 1.0-23`.

**Startup robustness:** before the first spawn the service waits
until the Wayland socket actually accepts connections (compositor
race at session start, e.g. re-login via LightDM). Bounded fast
retries plus a watchdog cover the rest — a dead `swaybg` on any
output is detected and re-applied, not only a total loss.

**Package:** `packaging/build-deb.sh` → cargo-deb.
