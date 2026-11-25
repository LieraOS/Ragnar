# TODO — ragnar-wallpaper

## Open

- [ ] Remove background + crystal_dock_helper from the daemon
      sources once `ragnar-core` builds without the background
      service (with the daemon split)
- [ ] Unit test for the `picture-options` → swaybg mode table
- [ ] Monitor refresh: respawn swaybg on output topology changes
      (hotplug)

## Done

- [x] Animated wallpaper: `wallpaper-animated` flag file
      (`true`/`false`, monitored via gio::FileMonitor) — `true` +
      image choice spawns one `ragnar-livewall --image <path>`
      process covering all monitors instead of per-output swaybg;
      spawn failure or flag `false` → normal swaybg set
- [x] Watch `livewall-effects.ini` alongside the flag — edits
      from the ragnar-settings effects editor respawn a running
      livewall with new shader parameters
