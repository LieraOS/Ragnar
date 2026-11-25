# ragnarok

Easter Egg mit echtem Nutzen: `ragnarok` zeigt alle Fehler, die
das System aufgezeichnet hat — journald-Error-Prioritäten,
vermischt mit den klassischen `/var/log`- und User-Xorg-Logs,
die journald nie sieht — gruppiert nach Quelle, sortiert nach
Fehleranzahl.

Antwortet auch auf `ragnarock` und `ragnarök` (Symlinks).

```
ragnarok                      alle Fehler, aktueller Boot
ragnarok lightdm              nach Quelle gefiltert
ragnarok -b -2                Boot vor zwei Booten
ragnarok --all                alle aufgezeichneten Boote
ragnarok lightdm -o out.txt   Report zusätzlich als Textdatei
```

Wird mit `ragnar-core` ausgeliefert — existiert nur auf einem
Ragnar-System, und genau darum geht es beim Easter Egg.

[English version](README.md)
