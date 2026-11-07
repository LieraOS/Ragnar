# ragnar-brightness-helper

## Deutsch

Rust-Ersatz für den legacy `ragnar-brightness-helper` aus ragnar-core.
Wird von den Compositor-Keybinds (XF86MonBrightnessUp/Down) aufgerufen
und steuert die Hintergrundbeleuchtung über logind — unprivilegiert,
kein Root, kein /sys-Schreibzugriff nötig.

**Aufruf:** `ragnar-brightness-helper --up|--down [--step N] | --set PCT`
(Standard-Schrittweite 5 %).

**Technik:** `org.freedesktop.login1.Session.SetBrightness` auf dem
eigenen Session-Objekt (`/org/freedesktop/login1/session/self`).
Gerätesuche über `/sys/class/backlight`; auf Desktops/VMs ohne
Backlight meldet der Helper „no backlight device" (Exit 1), wie das
legacy Original.

**Paket:** `packaging/build-deb.sh` → cargo-deb. Ersetzt die legacy 
Binary in ragnar-core am selben Pfad (`Replaces: ragnar-core`).

## English

Rust replacement for the legacy `ragnar-brightness-helper` inside
ragnar-core. Invoked by the compositor keybinds
(XF86MonBrightnessUp/Down) and controls backlight via logind —
unprivileged, no root, no sysfs writes.

**Usage:** `ragnar-brightness-helper --up|--down [--step N] | --set PCT`
(default step 5 %).

**Tech:** `org.freedesktop.login1.Session.SetBrightness` on the
caller's own session object (`/org/freedesktop/login1/session/self`).
Device discovery via `/sys/class/backlight`; on desktops/VMs without
backlight it reports "no backlight device" (exit 1), same as the
legacy original.

**Package:** `packaging/build-deb.sh` → cargo-deb. Replaces the legacy
binary inside ragnar-core at the same path (`Replaces: ragnar-core`).
