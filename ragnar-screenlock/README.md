# ragnar-screenlock

## Deutsch

Rust-Ersatz für den Screenlock-Teil des legacy `ragnar-daemon`.
Eigenständiger Session-Dienst, der `org.lieraos.ragnar.Screenlock`
auf dem Session-Bus besitzt und die Idle-Orchestrierung übernimmt.

**D-Bus-Methoden** (`/org/lieraos/ragnar/Screenlock`):
- `Lock()` — startet den Locker (`gtklock -d` mit Config/CSS-Auflösung
  über `~/.config`, `/etc`, `/usr/share` in `ragnar-desktop/`; Fallback
  `swaylock` mit Wallpaper aus `picture-uri`)
- `Dim()` / `Undim()` — Idle-Helligkeit über sysfs + logind
  `SetBrightness` (wie `ragnar-brightness-helper`)
- `ScreenPowerSuspend()` / `ScreenPowerResume()` — Schnittstellen-
  Parität (wlopm schaltet die Ausgänge)
- `OnBattery()` — AC/Akku-Zustand via `/sys/class/power_supply`

**Idle-Orchestrierung:** baut die `swayidle`-Kommandozeile aus den
GSettings (`org.gnome.desktop.session` `idle-delay`,
`org.gnome.desktop.screensaver` `lock-enabled`/`lock-delay`,
`org.gnome.settings-daemon.plugins.power` `idle-dim`/`idle-brightness`/
`sleep-inactive-*`, `org.gnome.desktop.lockdown` `disable-lock-screen`)
und respawnt swayidle bei jeder Änderung — identisch zum legacy Verhalten.

**Start:** per XDG-Autostart (`org.lieraos.ragnar.Screenlock.desktop`).
Der legacy Daemon registriert den Namen ab `ragnar-core 1.0-22` nicht
mehr, sodass der Wechsel sauber ist.

**Paket:** `packaging/build-deb.sh` → cargo-deb.

## English

Rust replacement for the screenlock part of the legacy `ragnar-daemon`.
Standalone session service owning `org.lieraos.ragnar.Screenlock` on
the session bus and handling idle orchestration.

**D-Bus methods** (`/org/lieraos/ragnar/Screenlock`):
- `Lock()` — spawns the locker (`gtklock -d` with config/CSS lookup
  across `~/.config`, `/etc`, `/usr/share` under `ragnar-desktop/`;
  `swaylock` fallback using the `picture-uri` wallpaper)
- `Dim()` / `Undim()` — idle brightness via sysfs + logind
  `SetBrightness` (same as `ragnar-brightness-helper`)
- `ScreenPowerSuspend()` / `ScreenPowerResume()` — interface parity
  (wlopm powers the outputs)
- `OnBattery()` — AC/battery state via `/sys/class/power_supply`

**Idle orchestration:** builds the `swayidle` command line from the
GSettings (`org.gnome.desktop.session` `idle-delay`,
`org.gnome.desktop.screensaver` `lock-enabled`/`lock-delay`,
`org.gnome.settings-daemon.plugins.power` `idle-dim`/`idle-brightness`/
`sleep-inactive-*`, `org.gnome.desktop.lockdown` `disable-lock-screen`)
and respawns swayidle on every change — identical to the legacy
behaviour.

**Startup:** via XDG autostart
(`org.lieraos.ragnar.Screenlock.desktop`). The legacy daemon no longer
registers the name as of `ragnar-core 1.0-22`, so the handover is
clean.

**Package:** `packaging/build-deb.sh` → cargo-deb.
