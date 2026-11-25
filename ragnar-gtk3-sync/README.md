# ragnar-gtk3-sync

GTK3 settings.ini synchronisation daemon for the Ragnar desktop (LieraOS).

## Deutsch

Headless-Dienst, der den noch lebenden Teil des legacy `SettingsManager`
ersetzt. GTK3 liest keine GSettings — Theme-Änderungen in
`org.gnome.desktop.interface` erreichen GTK3-Programme nur, wenn sie in
`~/.config/gtk-3.0/settings.ini` gespiegelt werden.

Was der Dienst tut:

- Beobachtet `gtk-theme`, `icon-theme`, `cursor-theme`, `cursor-size` und
  schreibt sie in `[Settings]` der `settings.ini` (andere Einträge bleiben
  erhalten).
- Hält `org.lieraos.ragnar.panel` `dark-theme` und den freedesktop-Schlüssel
  `color-scheme` konsistent, falls ein Client nur eine Seite ändert.
- Debounce über 300 ms, damit gebündelte Schlüssel-Schreibvorgänge nicht
  jede Datei mehrfach neu schreiben.

Startet über `/etc/xdg/autostart/org.lieraos.ragnar.Gtk3Sync.desktop`.

## English

Headless daemon replacing the remaining live part of the legacy
`SettingsManager`. GTK3 does not read GSettings — theme changes in
`org.gnome.desktop.interface` only reach GTK3 applications when mirrored
into `~/.config/gtk-3.0/settings.ini`.

What the daemon does:

- Watches `gtk-theme`, `icon-theme`, `cursor-theme`, `cursor-size` and
  writes them into the `[Settings]` section of `settings.ini` (other
  entries are preserved).
- Keeps `org.lieraos.ragnar.panel` `dark-theme` and the freedesktop
  `color-scheme` key consistent when a client only changes one side.
- 300 ms debounce so batched key writes do not rewrite the file for
  every single key.

Autostarted via `/etc/xdg/autostart/org.lieraos.ragnar.Gtk3Sync.desktop`.
