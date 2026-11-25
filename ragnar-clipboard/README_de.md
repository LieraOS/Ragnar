# ragnar-clipboard

Clipboard-Manager für den Ragnar-Desktop — Session-Komponente
(in `ragnar-core` enthalten), kein separates Programm.

## Bestandteile

- **Daemon** (`ragnar-clipboard daemon`, Autostart): beobachtet die
  Zwischenablage über das `wlr/ext-data-control`-Wayland-Protokoll
  und legt jede Auswahl in eine persistente History
  (`~/.local/share/ragnar-clipboard/`). Beendet sich die Quell-App,
  stellt der Daemon den neuesten Eintrag wieder bereit — die
  Zwischenablage überlebt also die App, aus der kopiert wurde
  (auf Wayland sonst nicht gegeben).
- **Picker** (`ragnar-clipboard pick`): GTK4-Fenster mit Verlauf,
  Suche und Enter/Doppelklick-Auswahl — per `Super+V` in der
  labwc-`rc.xml` angebunden. Das eigentliche Setzen übernimmt der
  Daemon, damit der Inhalt das Schließen des Pickers überlebt.
- **CLI**: `ragnar-clipboard list` / `clear`.

## Verhalten

- Behandelt Text und `image/png` (8 MiB pro Typ, 100 Einträge).
- Inhalte mit Password-Manager-Hint (`x-kde-passwordManagerHint`)
  werden grundsätzlich nicht gespeichert.
- Die History liegt als `index.jsonl` + ein Blob je MIME-Typ auf
  der Platte — überlebt Reboots und Session-Neustarts.

GPL-3.0-or-later — Jörg Schröder / LieraOS Project
