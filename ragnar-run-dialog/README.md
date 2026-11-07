# ragnar-run-dialog

## Deutsch

Launcher-Overlay für den Ragnar-Desktop (LieraOS) — der Ersatz für
den alten legacy `ragnar-run-dialog` aus ragnar-core.

Wird per Tastenkürzel (Alt+F2, bereits in der labwc-`rc.xml`
verdrahtet) gestartet, öffnet ein zentriertes Suchfeld am oberen
Bildschirmrand und beendet sich nach dem Start bzw. Abbrechen.

**Funktionen:**

- App-Liste aller installierten Desktop-Einträge (Icon, Name,
  Beschreibung), beim Öffnen alphabetisch gefüllt
- Live-Filter: Treffer im Namen schlagen Beschreibung, Keywords und
  Exec schlagen nur noch „nichts" — Präfix-Treffer zuerst
- Fallback-Zeile „Befehl ausführen" für freie Eingaben; erst
  `GAppInfo::create_from_commandline`, dann `sh -c` für Shell-Syntax
- Tastaturbedienung: ↑/↓ navigieren, Enter startet, Esc schließt;
  Fokusverlust schließt ebenfalls
- Erscheint auf dem fokussierten Monitor (Compositor wählt den
  Output, kein Pinning)

**Technik:** Rust + GTK4/libadwaita, `wlr-layer-shell`
Overlay-Surface mit exklusivem Keyboard-Grab — reines
Standard-Protokoll, läuft unter labwc und später unverändert unter
dem eigenen Ragnar-Kompositor.

**Theming:** eingebettetes `data/run-dialog.css`, übersteuerbar via
`~/.config/ragnar-desktop/run-dialog.css` (USER-Priorität, wie bei
den anderen Ragnar-Werkzeugen).

**Paket:** `packaging/build-deb.sh` → cargo-deb. Das Deb ersetzt die
alte legacy Binary aus ragnar-core am selben Pfad (`Replaces:
ragnar-core`); bis ragnar-core die Datei selbst entfernt, bleibt
dieser Mechanismus nötig.

## English

Launcher overlay for the Ragnar desktop (LieraOS) — replacement for
the legacy legacy `ragnar-run-dialog` inside ragnar-core .

Started by the compositor keybind (Alt+F2, already wired in the
labwc `rc.xml`), shows a centered search field at the top edge of
the screen and exits after launching or cancelling.

**Features:**

- App list of every installed desktop entry (icon, name,
  description), filled alphabetically on open
- Live filter: name hits outrank description, keywords and exec;
  prefix matches come first
- Fallback "Run command" row for free-form input; tries
  `GAppInfo::create_from_commandline` first, `sh -c` for shell
  syntax
- Keyboard: ↑/↓ navigate, Enter launches, Esc closes; losing focus
  closes too
- Appears on the focused monitor (compositor picks the output, no
  pinning)

**Tech:** Rust + GTK4/libadwaita, `wlr-layer-shell` overlay surface
with an exclusive keyboard grab — standard protocol only, runs under
labwc today and unchanged under the future Ragnar compositor.

**Theming:** embedded `data/run-dialog.css`, overridable via
`~/.config/ragnar-desktop/run-dialog.css` (USER priority, same as
the other Ragnar tools).

**Package:** `packaging/build-deb.sh` → cargo-deb. The deb replaces
the legacy legacy binary from ragnar-core at the same path
(`Replaces: ragnar-core`); keep this until ragnar-core stops
shipping the file itself.
