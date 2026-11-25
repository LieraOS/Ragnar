# ragnar-automount

## English

Session automounter for the Ragnar desktop. Mounts every mountable
volume that is not mounted yet — once at session start and again on
each hot-plug event announced by `gio::VolumeMonitor`. This is a
system responsibility, not the file manager's: Valkyrie only lists
devices, mounting happens here.

The kill switch is the GSettings key
`org.lieraos.ragnar.automount enabled`, toggled live by the
"Removable media" panel in Ragnar Settings. The schema ships in
`data/org.lieraos.ragnar.automount.gschema.xml` and is installed to
`/usr/share/glib-2.0/schemas/` by the `ragnar-settings` package
(its `postinst` runs `glib-compile-schemas`). When the schema is
absent the daemon defaults to enabled.

The key is re-read on every event, so toggling the switch in Settings
applies immediately — no session restart needed.

## Deutsch

Session-Automounter für den Ragnar-Desktop. Mountet jedes mountbare
Volume, das noch nicht eingehängt ist — einmal beim Sitzungsstart und
erneut bei jedem Hot-Plug-Ereignis, das der `gio::VolumeMonitor`
meldet. Das ist eine Systemaufgabe, nicht die des Dateimanagers:
Valkyrie listet Geräte nur auf, das Mounten passiert hier.

Der Schalter ist der GSettings-Key
`org.lieraos.ragnar.automount enabled`, der live über das Panel
„Wechseldatenträger" in Ragnar Settings umgelegt wird. Das Schema
liegt in `data/org.lieraos.ragnar.automount.gschema.xml` und wird vom
`ragnar-settings`-Paket nach `/usr/share/glib-2.0/schemas/`
installiert (dessen `postinst` führt `glib-compile-schemas` aus).
Fehlt das Schema, ist der Daemon standardmäßig aktiviert.

Der Key wird bei jedem Ereignis neu gelesen — ein Umschalten in den
Settings greift sofort, kein Sitzungsneustart nötig.

## Build

```sh
cargo build --release
```
