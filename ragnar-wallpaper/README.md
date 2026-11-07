# ragnar-wallpaper

## Deutsch

Rust-Ersatz für den Hintergrund-Teil des legacy `ragnar-daemon`.
Eigenständiger Session-Dienst: liest `org.gnome.desktop.background`
(`picture-uri`, `picture-options`, `primary-color`), hält einen
`swaybg`-Prozess am Laufen und respawnt ihn bei jeder Änderung —
inklusive kurzem Übergabe-Fenster, damit der Wechsel nicht flackert.

**Modi:** GNOME `picture-options` → swaybg (`wallpaper`→`tile`,
`centered`→`center`, `scaled`→`fit`, `stretched`→`stretch`,
`zoom`/`spanned`→`fill`). Ohne Bild oder bei `options=none`:
`swaybg -m solid_color` mit `primary-color` — auf Laptops damit
auch reine Farb-Hintergründe.

**Greeter-Sync:** schreibt die Wahl über den System-Bus in
AccountsService (`User.SetBackgroundFile` + DisplayManager-
Extension, LightDM-Konvention) — der Login-Screen zeigt dasselbe
Bild. Best-effort, nie fatal.

**Start:** XDG-Autostart (`org.lieraos.ragnar.Wallpaper.desktop`).
Der legacy Daemon übernimmt den Job ab `ragnar-core 1.0-23` nicht mehr.

**Paket:** `packaging/build-deb.sh` → cargo-deb.

## English

Rust replacement for the background part of the legacy
`ragnar-daemon`. Standalone session service: reads
`org.gnome.desktop.background` (`picture-uri`, `picture-options`,
`primary-color`), keeps a `swaybg` process alive and respawns it on
every change — with a short hand-over so switches don't flicker.

**Modes:** GNOME `picture-options` → swaybg (`wallpaper`→`tile`,
`centered`→`center`, `scaled`→`fit`, `stretched`→`stretch`,
`zoom`/`spanned`→`fill`). Without an image or `options=none`:
`swaybg -m solid_color` using `primary-color`.

**Greeter sync:** writes the choice into AccountsService over the
system bus (`User.SetBackgroundFile` plus the DisplayManager
extension, LightDM convention) — the greeter shows the same image.
Best-effort, never fatal.

**Startup:** XDG autostart (`org.lieraos.ragnar.Wallpaper.desktop`).
The legacy daemon no longer does this job as of `ragnar-core 1.0-23`.

**Package:** `packaging/build-deb.sh` → cargo-deb.
