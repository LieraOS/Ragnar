# ragnar-greeter

Animierter LightDM-Greeter für den Ragnar-Desktop. Ersetzt den
lightdm-gtk-greeter durch einen GTK4-Vollbild-Greeter, der das
LIERA-Artwork durch die gesamte Boot-Kette zieht: derselbe
Hintergrund wie bei GRUB und Plymouth, dieselbe Blitz-/Glow-
Umgebungsanimation wie der Plymouth-Splash und ein Login-Panel
unterhalb des Ornament-Bands — wie das GRUB-Menü.

## Funktionen

- Vollbild-Animation über alle Monitore: cover-skalierter
  LIERA-Hintergrund, ein goldener Beam auf zufälligen Pfaden,
  Blitze (links/rechts/mitte, gelegentlich Doppelschläge) und
  funkelnde Sterne im Himmel-Band
- Benutzername: immer frei eintippbar, zusätzlich ein
  Benutzerlisten-Button, wenn der Daemon Konten kennt
  (respektiert `hide-users`)
- Passwortfeld mit Sichtbar-Umschalter; Enter im Namensfeld
  springt ins Passwortfeld
- Sitzungs-Dropdown (automatisch versteckt bei nur einer Sitzung)
- Power-Aktionen (Bereitschaft / Ruhezustand / Neustart /
  Herunterfahren) — nur die vom Daemon gemeldeten
- Uhr + Datum als Overlay, in dezentem Gold
- Timed-Autologin gemäß Daemon-Hint
- Deutsche UI-Strings mit englischem gettext-Katalog (`po/en.po`)

## Funktionsweise

Der Greeter läuft als `greeter-session` in LightDMs eigenem
X-Server — ohne Fenstermanager, deshalb wird das Fenster direkt
auf die Bounding-Box aller Monitore gesetzt; das Login-Panel
bleibt über das Anker-Rechteck der Szene auf dem primären
Monitor zentriert. Die Authentifizierung läuft
über `liblightdm-gobject-1` und eine kleine handgeschriebene
FFI-Schicht (`src/ffi.rs`); das Crate linkt gegen den versionierten
Soname, `liblightdm-gobject-dev` wird zum Bauen nicht gebraucht.

## Bauen

```sh
cargo build --release
cargo test
```

Laufzeitabhängigkeiten: `liblightdm-gobject-1-0`, GTK4.

## Vorschau ohne LightDM

```sh
cargo run -- --preview        # Fenster, kein Daemon nötig
RAGNAR_GREETER_ARTWORK=/pfad  # Artwork-Verzeichnis überschreiben
```

## Installation / Paket

`cargo deb` erzeugt das `.deb` mit:

- `/usr/bin/ragnar-greeter`
- `/usr/share/ragnar-greeter/artwork/*`
- `/usr/share/xgreeters/ragnar-greeter.desktop`
- `/etc/lightdm/lightdm.conf.d/50-ragnar-greeter.conf`
  (`greeter-session=ragnar-greeter`)
- englischem Locale-Katalog

Auf der Live-ISO ändert sich nichts — Autologin überspringt den
Greeter. Auf installierten Systemen erscheint er beim nächsten Boot.
