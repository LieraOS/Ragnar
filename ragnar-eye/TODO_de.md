# TODO — ragnar-eye

[English version](TODO.md)

## Offen

- Verbleibende exotische Decoder portieren: MSP, VICAR,
  VIFF, ILBM, MIFF, MNG, XWD, WBMP, PCX, SGI RGB, JP2/J2K
  (openjpeg). Erst Extensions wieder in
  `supported_extensions()` aufnehmen, wenn ein Decoder läuft.
- Animierte GIF/APNG: aktuell nur erstes Frame; Frame-Stepping
  ergänzen.
- Die Pfade für DjVu/FITS/JXL/libraw/HEIF gegen echte Dateien
  verifizieren — auf diesem Rechner gibt es keine
  Testdateien, diese Formate sind ungetestet.
- Skaliertes Dekodieren für Thumbnails (z. B. JPEG-DCT-
  Skalierung) statt Vollauflösung plus Verkleinerung.
- Zoom zum Mauszeiger — das Rad zoomt derzeit ums Zentrum.
- EXIF-Metadaten im Eigenschaften-Dialog (Kamera,
  Aufnahmedatum).
- Single-Instance-Verhalten: ein zweiter Start soll die Datei
  an die laufende Instanz reichen statt einen parallelen
  Prozess zu öffnen.
- Mehrseitige TIFF/DjVu-Navigation.
- Slideshow und Vergleichsansicht — entscheiden, ob sie
  übernommen werden.
- Paketplatzierung: entscheiden, ob ragnar-eye ins Core-Image
  oder als empfohlenes Standalone-Paket kommt.
