# TODO — ragnar-wallpaper

## Offen

- [ ] background + crystal_dock_helper aus den Daemon-Sourcen
      entfernen, sobald `ragnar-core` ohne den Background-Service
      gebaut wird (mit der Daemon-Aufsplitterung)
- [ ] Unit-Test für die `picture-options` → swaybg-Mode-Tabelle
- [ ] Monitor-Refresh: swaybg neu spawnen, wenn sich die
      Output-Topologie ändert (Hotplug)

## Erledigt

- [x] Animiertes Hintergrundbild: Flag-Datei
      `wallpaper-animated` (`true`/`false`, überwacht per
      gio::FileMonitor) — `true` + Bildwahl spawnt einmalig
      `ragnar-livewall --image <Pfad>` für alle Monitore statt
      swaybg pro Output; Spawn-Fehler oder Flag `false` →
      normales swaybg-Set
- [x] `livewall-effects.ini` zusätzlich zum Flag beobachten —
      Änderungen aus dem ragnar-settings-Effekt-Editor starten
      ein laufendes livewall mit neuen Shader-Parametern neu
