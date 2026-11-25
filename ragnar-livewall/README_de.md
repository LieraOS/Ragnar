# ragnar-livewall

Animiertes Hintergrundbild für den Ragnar-Desktop (Liera).

Ein kleiner `zwlr_layer_shell`-Client, der ein Hintergrundbild
auf die **Hintergrund-Ebene** des Compositors legt — denselben
Platz, den `swaybg` belegt — und langsame, prozedurale
Polarlicht-Schleier über den Himmelsbereich zeichnet. Da nur
Standard-Wayland-Protokolle benutzt werden, läuft es auf jedem
Layer-Shell-Compositor: heute labwc, später Odin.

## Funktionen

- Eine Hintergrund-Surface pro Monitor (Multi-Monitor-fähig)
- Prozedurale Aurora in einem OpenGL-Fragment-Shader gerendert
  (fbm-Rausch-Vorhänge mit wandernden Strahlen) in einer
  GtkGLArea — fällt auf das statische Bild zurück, falls GL
  scheitert. 15 fps reichen: ein echtes Polarlicht kriecht,
  der GPU-Aufwand bleibt minimal
- **Automatische Pause**: Sobald ein Fenster im Vollbild läuft
  (Spiele, Videos), friert die Animation auf null GPU-Kosten ein —
  erkannt über `zwlr_foreign_toplevel_v1` auf einer zweiten
  Wayland-Verbindung
- Hüttenlampen-Flackern auf dem Standard-Motiv: Fensterlicht
  schimmert, flackert gelegentlich oder geht kurz aus
- **Konfigurierbare Effekte** (Glow, Sterne, Nebel, Schnee, Regen,
  Glühwürmchen, Wolkenschatten) pro Hintergrundbild über
  `~/.config/ragnar-settings/livewall-effects.ini` — jeder ist ein
  schaltbarer Shader-Block; alle außer dem einzelnen Glow-Band
  akzeptieren bis zu vier Regionen.
  Regionen und Parameter sind Shader-Uniforms; der
  Effekt-Editor in ragnar-settings schreibt die Datei und seine
  GL-Vorschau rendert exakt das unter
  `/usr/share/ragnar-livewall/` installierte GLSL
- Kein Tastaturfokus, kein Fensterrahmen — reines Hintergrundbild

## Benutzung

```sh
ragnar-livewall                # Standard-Hintergrundbild
ragnar-livewall --image P.png  # anderes Bild animieren
```

Aus dem Terminal starten; `Strg-C` beendet es. In einer
Ragnar-Sitzung spawnt der `ragnar-wallpaper`-Daemon es
automatisch (ein Prozess für alle Monitore), solange der Schalter
„Animiertes Hintergrundbild" in ragnar-settings an ist — die App
registriert keinen D-Bus-Namen (`ApplicationFlags::NON_UNIQUE`),
damit gespawnte Kinder nicht vorzeitig enden.

## Bauen

```sh
cargo build --release
```

Laufzeit-Abhängigkeiten: GTK 4.16+, ein Compositor mit
`zwlr_layer_shell_v1` (v4) und optional
`zwlr_foreign_toplevel_manager_v1` für die Auto-Pause.

## Geplant

- Pause bei Bildschirmsperre / Akkubetrieb
- Effekt-Konfiguration pro Hintergrundbild (Sidecar-Datei) +
  eigener Animations-Editor mit Live-Vorschau in ragnar-settings
- Live-Build-Integration nach Freigabe

## Lizenz

GPL-3.0-or-later. Copyright (C) Jörg Schröder.
