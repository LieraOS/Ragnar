# ragnar-clipboard

Clipboard manager for the Ragnar desktop — a session component
(shipped in `ragnar-core`), not a separately installed app.

*Deutsche Version: [README_de.md](README_de.md)*

## Parts

- **Daemon** (`ragnar-clipboard daemon`, autostarted): watches the
  clipboard through the `wlr/ext-data-control` Wayland protocol and
  stores every selection in a persistent history
  (`~/.local/share/ragnar-clipboard/`). When the source application
  exits, the daemon re-provides the newest entry — clipboard
  contents therefore survive the application they were copied from
  (not a given on Wayland otherwise).
- **Picker** (`ragnar-clipboard pick`): GTK4 history window with
  search and Enter/double-click selection — bound to `Super+V` in
  the labwc `rc.xml`. The daemon performs the actual set so the
  content outlives the picker.
- **CLI**: `ragnar-clipboard list` / `clear`.

## Behaviour

- Handles text and `image/png` (8 MiB per type, 100 entries).
- Offers carrying the password-manager hint
  (`x-kde-passwordManagerHint`) are never stored.
- History lives on disk as `index.jsonl` + one blob per MIME type —
  survives reboots and session restarts.

GPL-3.0-or-later — Jörg Schröder / LieraOS Project
