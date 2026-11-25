# Ragnar Eye

Schneller GTK4-Bildbetrachter für den Ragnar-Desktop mit
klassischem Dreispalten-Layout: Verzeichnisbaum links,
betrachtetes Bild in der Mitte, Verzeichnis-Thumbnails
rechts — gebaut für Geschwindigkeit und breite
Formatunterstützung.

[English version](README.md)

## Funktionen

- **Dreispalten-Layout** — Dateibaum links, betrachtetes Bild in
  der Mitte, Thumbnails des aktuellen Ordners rechts. Ein Klick
  auf ein Thumbnail zeigt das Bild; die Ordnerauswahl füllt die
  Leiste neu.
- **Freier Zoom** — Rad-Zoom bis 25600 %, Ziehen zum
  Verschieben, Fit-Modi (`F`/`0` = Fenster, `1` = 100 %),
  Nearest-Neighbour ab 400 % für knackige Pixel.
- **Schnelle Navigation** — Pfeile/Bildtasten/Leertaste durchs
  Verzeichnis; Nachbarbilder werden im Hintergrund vorgeladen.
  Async-Dekodierung hält die Oberfläche auch bei 60-MB-Dateien
  flüssig.
- **Formate** — PNG, JPEG, GIF, WebP, TIFF, BMP, TGA, ICO/CUR,
  PNM, QOI, DDS, EXR, HDR, farbfeld, APNG über `image`; PSD/PSB
  über `psd`; KRA/ORA als ZIP-Container; Kamera-RAW (CR2/CR3,
  NEF, ARW, DNG u.v.m.) über libraw; HEIC/HEIF/AVIF über libheif;
  JPEG XL über libjxl; DjVu über libdjvulibre; FITS über cfitsio;
  EPS/PS/AI über ghostscript; dazu portierte Decoder für PFM/PHM,
  Sun RAS, MTV, XBM, XPM.
- **Leichte Filter** — Graustufen, Sepia, Invertieren,
  Automatisch verbessern (Histogramm-Spreizung), Rote-Augen-
  Reduktion; „Original" stellt das unveränderte Bild wieder her.
  Ein Betrachter, kein Editor.
- **Thumbnail-Cache** — unter `~/.cache/ragnar-eye/thumbs/`,
  schlüsselstabil über Pfad+Zeit+Größe.
- Optionale Codec-Bibliotheken werden zur Laufzeit geladen —
  fehlt eine, ist nur dieses Format nicht verfügbar.

## Aufruf

```
ragnar-eye bild.cr2
ragnar-eye
```

Tasten: `←/→` bzw. `Bild↑/↓` navigieren, `Leertaste` weiter,
`+/-` Zoom, `0`/`F` einpassen, `1` Originalgröße.

## Bau

```
cargo build --release
```

Optionale Laufzeit-Codecs (pro Format): `libraw25`, `libheif1`,
`libjxl0.11`, `libavif16`, `libdjvulibre21`, `libcfitsio10`,
`ghostscript`.

## Lizenz

GPL-3.0-or-later — © Jörg Schröder, LieraOS Project
