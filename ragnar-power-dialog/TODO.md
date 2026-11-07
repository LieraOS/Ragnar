# TODO — ragnar-power-dialog

**Deutsch**

- [x] Dialog-Fenster mit Aktions-Kacheln (GTK4/libadwaita)
- [x] Echte Backends: Screenlock-D-Bus, ragnar-session-quit, systemctl
- [x] Sichtbarkeit pro Aktion via gsettings (`show-*`)
- [x] Hardware-Fähigkeiten prüfen (logind Can*)
- [x] User-CSS-Override (~/.config/ragnar-desktop/power-dialog.css)
- [ ] Bestätigungs-Schritt vor Shutdown/Reboot (optional, gsettings-Key)
- [ ] Countdown/„Sitzung endet in X s" wie bei GNOME/KDE (optional)
- [ ] Theming-Verwaltung aus Ragnar Settings → Aussehen anbinden
- [ ] Icon-Fallback prüfen, falls `system-hibernate-symbolic` im
      Icon-Theme fehlt

**English**

- [x] Dialog window with action tiles (GTK4/libadwaita)
- [x] Real backends: Screenlock D-Bus, ragnar-session-quit, systemctl
- [x] Per-action visibility via gsettings (`show-*`)
- [x] Hardware capability checks (logind Can*)
- [x] User CSS override (~/.config/ragnar-desktop/power-dialog.css)
- [ ] Optional confirmation step before shutdown/reboot (gsettings key)
- [ ] Optional "session ends in X s" countdown like GNOME/KDE
- [ ] Wire theming management into Ragnar Settings → Appearance
- [ ] Icon fallback if `system-hibernate-symbolic` is missing in the
      icon theme
