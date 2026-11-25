# ragnar-greeter — TODO

## Offen

- Countdown-Anzeige für Timed-Autologin („Anmeldung als X in N s")
- Optionaler Bildschirmtastatur-Button (Barrierefreiheit)
- Tastatur: Escape sollte eine laufende Anmeldung abbrechen
  (Fokusreihenfolge und Enter-Weiterschaltung sind fertig)
- Power-Buttons auf echter Hardware verifizieren — in QEMU
  meldete der Daemon keine Power-Fähigkeiten, daher nichts sichtbar

## Erledigt

- Handgeschriebene liblightdm-gobject-1-FFI-Schicht (kein
  Dev-Paket nötig — linkt den versionierten Soname)
- Animiertes LIERA-Backdrop über alle Monitore: cover-skalierter
  Hintergrund, goldener Beam auf zufälligen Pfaden, Blitze
  (Zonen links/rechts/mitte, Doppelschläge), funkelnde Sterne;
  Panel auf dem primären Monitor verankert
- Login-Panel unter dem Ornament-Band (Position wie GRUB-Menü)
- Eintippbarer Benutzername + Benutzerlisten-Button,
  Sitzungsauswahl
- Power-Leiste nach Daemon-Fähigkeiten
- Uhr + Datum als Overlay
- Tastaturfluss: Fokus startet im Namensfeld, Enter springt
  ins Passwortfeld
- `--preview`-Modus + `RAGNAR_GREETER_ARTWORK`-Umgebungsvariable
- lightdm.conf.d-Snippet wählt die Greeter-Session
- Verifiziert gegen echten Daemon via `lightdm --test-mode`
  und Ende-zu-Ende in QEMU auf der Live-ISO
