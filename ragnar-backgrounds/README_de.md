# ragnar-backgrounds

Hintergrundbild-Paket für den Ragnar-Desktop. Bilder in `data/` werden installiert unter `/usr/share/backgrounds/ragnar/` und
registriert sie über `gnome-background-properties`, damit sie
im Erscheinungsbild von ragnar-settings auftauchen.

Aktueller Satz (4K, LIERA-Arktik-Artwork):

- `aurora-fjord.png` — Polarlicht über einem norwegischen Fjord,
  zugleich das Standard-Desktophintergrundbild (wird zusätzlich
  als `lieraos-default.png` / `lightdm.png` vom Live-Build-
  Overlay installiert)
- `raven-fjord.png` — Aurora-Rabe über derselben Fjord-Szenerie

Weitere Bilder einfach in `data/` ablegen und neu bauen.
`default.jpg` wird immer in der XML referenziert, kommt aber
aus ragnar-core, damit der Picker nie leer ist.

## Bau

```
./build-deb.sh
```

erzeugt `ragnar-backgrounds_<version>_all.deb`. Neues Bild?
Einfach in `data/` ablegen und neu bauen — die XML wird
automatisch neu erzeugt.

[English version](README.md)
