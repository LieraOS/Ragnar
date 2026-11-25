# ragnar-livewall — TODO

## Offen

- [ ] Aurora-Overlay auf dem Produktiv-Desktop testen
      (Feinschliff: Schleierdichte, Geschwindigkeit, Farbmischung)
- [ ] Auto-Pause an einem echten Vollbild-Fenster verifizieren
- [ ] Aurora-Performance/Optik auf AMD- und Intel-GPUs prüfen —
      bisher nur auf NVIDIA validiert (OpenGL ES 3.2)
- [ ] Effekt-Konfiguration erweitern: Glow-Rechtecke mit
      Flacker-Modi — gleicher Uniform-Mechanismus wie
      Polarlicht/Sterne/Nebel
- [ ] Fest verdrahtetes aurora-fjord-Lampenflackern
      (CSS-Overlays) als konfigurierbare Glow-Spots in den
      Shader überführen
- [ ] Pause bei Bildschirmsperre und Akkubetrieb
- [ ] Standard-Bildpfad aus dem installierten
      ragnar-backgrounds-Paket beziehen statt fest codiert
- [ ] .deb-Paketierung + Live-Build-Integration nach Freigabe
- [ ] Autostart-Eintrag für die Ragnar-Sitzung (sobald
      standardmäßig aktiv)

## Erledigt

- [x] Hintergrund-Surface pro Monitor (zwlr_layer_shell)
- [x] GLSL-Fragment-Shader-Aurora über dem Bild, 15 fps
      (GtkGLArea, OpenGL ES / Desktop-GL Dual-Dialekt)
- [x] Hüttenlampen-Flackern-Overlays auf dem Standard-Motiv
- [x] Vollbild-Auto-Pause via zwlr_foreign_toplevel_v1
- [x] `--image`-Argument zum Testen beliebiger Bilder
- [x] Gespiegelte, gedimmte Aurora im Wasserband + Luminanz-Maske,
      damit dunkle Wolken kein Glühen zeigen
- [x] `ApplicationFlags::NON_UNIQUE` — vom Daemon gespawnte
      Kinder dürfen nie per Single-Instance-Forwarding enden
- [x] Integration: ragnar-wallpaper spawnt livewall für alle
      Monitore, solange `wallpaper-animated` `true` ist
- [x] Konfigurierbare Effekte pro Bild über
      livewall-effects.ini — uniform-getriebene Shader-Blöcke;
      das Polarlicht ist ein schaltbarer Effekt mit eigener
      Region/Stärke, Sterne und Nebel nehmen bis zu vier
      `;`-getrennte Regionen; Schnee, Regen, Glühwürmchen und
      Wolkenschatten kamen in 0.1.2 dazu — gleicher
      Uniform-Mechanismus
- [x] Shader-Quellen als installierte Dateien (include_str! ==
      .deb-Inhalt) — geteilt mit der GL-Vorschau in
      ragnar-settings
