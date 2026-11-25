# ragnar-wallpaper

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

**Animiertes Hintergrundbild (Beta):** beobachtet
`~/.config/ragnar-settings/wallpaper-animated` (`true`/`false`,
geschrieben vom Erscheinungsbild-Panel in ragnar-settings). Steht
das Flag auf `true` und die Wahl ist ein Bild, ersetzt ein einziger
`ragnar-livewall`-Prozess die swaybg-Spawns pro Output — er belegt
die Hintergrund-Ebene auf jedem Monitor. Flag aus oder
Spawn-Fehler → normales swaybg-Set. Derselbe Watcher beobachtet
auch `livewall-effects.ini` (Effekt-Parameter pro Bild, geschrieben
vom Effekt-Editor in ragnar-settings), damit Änderungen ein
laufendes livewall mit neuen Werten neu starten. `ragnar-livewall` muss mit
`ApplicationFlags::NON_UNIQUE` laufen: GApplication-Single-Instance-
Forwarding würde jedes gespawnte Kind sofort beenden und die
Respawn-Logik verklemmen.

**Greeter-Sync:** schreibt die Wahl über den System-Bus in
AccountsService (`User.SetBackgroundFile` + DisplayManager-
Extension, LightDM-Konvention) — der Login-Screen zeigt dasselbe
Bild. Best-effort, nie fatal.

**Startrobustheit:** Vor dem ersten Spawn wartet der Dienst, bis der
Wayland-Socket tatsächlich Verbindungen annimmt (Compositor-Race beim
Session-Start, z. B. Re-Login über LightDM). Danach decken begrenzte
Schnell-Retries plus ein Watchdog den Rest ab — ein toter `swaybg`
pro Output wird erkannt und neu angewendet, nicht nur ein Komplett-
Ausfall.

**Start:** XDG-Autostart (`org.lieraos.ragnar.Wallpaper.desktop`).
Der legacy Daemon übernimmt den Job ab `ragnar-core 1.0-23` nicht mehr.

**Paket:** `packaging/build-deb.sh` → cargo-deb.
