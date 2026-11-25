# ragnar-backgrounds

Background image package for the Ragnar desktop. Images dropped
into `data/` are shipped under `/usr/share/backgrounds/ragnar/`
and registered via `gnome-background-properties`, so they appear
in the appearance section of ragnar-settings.

Current set (4K, LIERA arctic artwork):

- `aurora-fjord.png` — aurora borealis over a Norwegian fjord,
  also the stock desktop wallpaper (installed additionally as
  `lieraos-default.png` / `lightdm.png` by the live-build overlay)
- `raven-fjord.png` — aurora raven over the same fjord scenery

Drop further images into `data/` and rebuild. `default.jpg` is
always referenced in the XML but shipped by ragnar-core, so the
picker never comes up empty.

## Build

```
./build-deb.sh
```

produces `ragnar-backgrounds_<version>_all.deb`. To add an
image, drop it into `data/` and rebuild — the XML is
regenerated automatically.

[Deutsche Version](README_de.md)
