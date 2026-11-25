# TODO — ragnar-screenlock

## Deutsch

- [ ] Screenlock-Dienst auch aus den Daemon-Sourcen entfernen, sobald
      `ragnar-core` ohne ihn gebaut wird (screenlock bleibt nur
      für `is_dimming` kompiliert — fällt mit der Daemon-Aufsplitterung)
- [ ] `ScreenPowerSuspend/Resume`: internen Screen-Off-State pflegen,
      falls künftige Dienste (OSD, Notifications) ihn brauchen
- [ ] Unit-Tests für die swayidle-Kommandozeilen-Generierung
- [ ] `Dim`/`Undim` ohne logind-Session: sysfs-Fallback

## English

- [ ] Drop the screenlock service from the daemon sources once
      `ragnar-core` builds without it (screenlock only stays
      compiled for `is_dimming` — goes away with the daemon split)
- [ ] `ScreenPowerSuspend/Resume`: keep an internal screen-off state
      if future services (OSD, notifications) need it
- [ ] Unit tests for the swayidle command-line generation
- [ ] `Dim`/`Undim` without a logind session: sysfs fallback
