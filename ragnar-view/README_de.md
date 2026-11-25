# ragnar-view

Desktop-Icons-Schicht für den Ragnar-Desktop (LieraOS). Ersetzt den
alten legacy/`ragnar-desktop-view` vollständig durch Rust + GTK4 +
gtk4-layer-shell — eine `Layer::Bottom`-Surface **pro Monitor**,
unterhalb aller Fenster. Jedes Icon gehört genau einem Ausgang:
die geteilte Layout-Map (Anschluss → Positionen in
`~/.config/ragnar-view/layout-<Anschluss>`) entscheidet, wo es
erscheint. Der Primärmonitor übernimmt alles Unzugewiesene
(inkl. Icons abgesteckter Monitore); die alte Datei `layout`
wird als seine Sektion geerbt.

Icons lassen sich über Monitorgrenzen ziehen — beim Drop gehört
das Icon dem Zielmonitor. „Neuer Ordner" / „Leeres Dokument"
landet am Klickpunkt auf dem Monitor, auf dem das Menü geöffnet
wurde.

Zeigt: Persönlicher Ordner, Papierkorb, eingebundene Laufwerke
(gio::VolumeMonitor) und den Inhalt von ~/Desktop.

Kontextmenüs (rechts lokalisiert, de/en):

- **Hintergrund:** Leeres Dokument / Neuer Ordner anlegen,
  Sortieren (Name, Datum, Größe, Typ), Im Dateimanager öffnen,
  Hintergrund / Anzeige / System → `ragnar-settings`.
- **Icon:** Öffnen, Öffnen mit… (empfohlene + alle Programme),
  Im Terminal öffnen, Umbenennen, Löschen. Löschen-Verhalten
  (endgültig mit Rückfrage = Standard, oder Papierkorb) und
  sichtbare Einträge stellt der Benutzer in Ragnar Settings →
  „Panel und Desktop" ein (GSettings `org.lieraos.ragnar.view`).
- Icons frei verschiebbar (Drag & Drop), Positionen bleiben
  gespeichert (`~/.config/ragnar-view/layout`). Untermenü
  „Symbolanordnung": Am Raster ausrichten (Standard), Frei
  platzieren, Automatisch anordnen. „Sortieren nach" (Name, Datum,
  Größe, Typ) ordnet die Icons einmalig neu ins Raster.
- Menüs zeigen immer vollständig, ohne Scrollen (GMenu-Sections
  lassen GTKs PopoverMenu ~13 px pro Section falsch messen — die
  Menüs sind deshalb flach aufgebaut).
- Mehrfachauswahl per Gummiband; markierte Icons lassen sich
  gemeinsam verschieben.
- Dialoge schließen sich bei Klick auf den freien Desktop.
- Main monitor = `org.lieraos.ragnar.panel primary-monitor-list`.

Aktualisiert sich automatisch über `gio::FileMonitor` auf
~/Desktop und VolumeMonitor-Signale.

Dateinamen-Effekte (optional, aus → identisches Rendern):
Icon-Beschriftungen werden vom `FxLabel`-Widget gezeichnet, das
`~/.config/ragnar-settings/font-effects.ini` [names] live liest
(Datei-Monitor) — wahlweise Helligkeits-Pulsieren oder eine über
die Glyphen laufende Farbwelle (Farben, Richtung, Stärke, Tempo
in Ragnar Settings → Erscheinungsbild → Schriften einstellbar).
