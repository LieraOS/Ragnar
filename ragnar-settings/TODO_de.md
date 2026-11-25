# Ragnar Settings TODO (deutsch)

## Phase A — Grundgerüst

- [x] Cargo-Workspace: GTK4 + libadwaita, englischer Code,
      i18n von Anfang an, Dateien <= 400 Zeilen, ein Modul
      pro Feature-Bereich.
- [x] Hub-Fenster: kategorisiertes Kachelraster + Suchfeld,
      Registry-Katalog als Kategorie-/Kachel-Modell.
- [x] Panel-Start-Abstraktion: eingebettetes Panel vs. externe
      App-Kachel (`Action::Panel` / `Action::Launch`).
- [x] Erstes echtes Panel: „Über dieses System" (os-release, Kernel,
      Speicher, Sitzung).
- [x] Verfügbarkeitsprüfung pro Kachel; nicht verfügbare Einträge
      werden ausgeblendet, sodass die App auf Nicht-Ragnar-
      Entwicklungsrechnern läuft.
- [x] .desktop-Datei + Icon — seit 1.0.4 ausgeliefert, erscheint als
      „Einstellungen" im Taskbar-Menü.
- [x] gettext-Kataloge + Workflow (1.0.12): `po/de.po` vollständig
      (495 Strings, msgfmt-sauber), `po/ragnar-settings.pot`-
      Vorlage, `build.rs` kompiliert jedes po/*.po nach
      `target/locale/`, cargo-deb liefert die `.mo` unter
      `/usr/share/locale/` aus; `LOCALEDIR`-Override für
      Entwicklungsläufe; Extraktions-/Merge-Befehle in der README
      dokumentiert. Englisch ist Quellsprache — kein en.po nötig.
      es/fr-Kataloge später.

## Phase B — native Panels

- [x] Personalisierung: appearance/-Panel — Modus, Akzentfarben,
      Design-/Cursorzeilen.
- [x] Schalter „Animiertes Hintergrundbild (Beta)" im
      Hintergrundbild-Panel — schreibt
      ~/.config/ragnar-settings/wallpaper-animated; der
      ragnar-wallpaper-Daemon spawnt dann ragnar-livewall statt
      swaybg.
- [x] Effekt-Editor (1.0.16): „Effekte…" im Hintergrundbild-Panel
      öffnet einen Dialog mit Live-GL-Minivorschau, die dasselbe
      installierte GLSL rendert wie ragnar-livewall; Sterne- und
      Nebel-Regionen werden direkt auf der Vorschau gezogen,
      Parameter per Slider; Speichern schreibt
      livewall-effects.ini, das ragnar-wallpaper sofort anwendet.
- [x] Glow als Effekt + Mehrfach-Regionen (1.0.17): das
      Polarlichtband (UI: „Glow") ist ein schaltbarer Effekt mit
      Region/Stärke wie die anderen; Sterne und Nebel nehmen bis zu vier
      Drag-Rechtecke je Effekt (Marching-Ants-Markierung),
      Zurücksetzen-Knopf pro Effekt, Schnee/Regen/Glühwürmchen/
      Wolkenschatten hinzugefügt — der Dialog rendert alle sieben
      Shader-Blöcke.
- [ ] Editor erweitern: Glow-Regionen — gleicher Mechanismus,
      braucht zuerst Shader-Unterstützung in ragnar-livewall.
- [x] Anzeige: display/-Panel (Anordnungs-Canvas, Auflösung,
      Ausrichtung, GPU-Sektion).
- [x] Klänge: sounds.rs schreibt Ravens settings.ini — die Datei
      ist die Schnittstelle.
- [x] Kaputter INI-Writer behoben (1.0.10): Klang-/Benachrichtigungs-
      Schreibzugriffe laufen über das getestete
      panels::ini::write_key.
- [x] Benachrichtigungs-Panel erweitert (1.0.11): Ereignis-Karte
      schaltet Ravens Watcher, Farb-Karte schreibt pro-Kategorie
      color_*-Keys; OSD-Zeilen nach panels/osd.rs.
- [x] Nachtlicht: gammastep-gestütztes Panel.
- [x] Aktualisierungen: Anzahl via apt-Simulation, pkexec-
      Auffrischen/Installieren, Auto-Upgrade-Drop-in mit explizitem
      origin=Debian,label=Debian-Muster.
- [x] Barrierefreiheit: Textgröße, Animationen reduzieren, hoher
      Kontrast — alles schema-abgesichert.
- [x] Gemeinsame Helfer: panels/common.rs + generisches
      panels/ini.rs.
- [x] Wechseldatenträger: Automount-Schalter + Live-Geräteliste
      mit Aushängen/Auswerfen via gio::VolumeMonitor.
- [x] Autostart-Ansicht in desktop.rs.
- [x] Standardanwendungen (xdg-mime); um Kalender- und
      Archiv-Kategorien erweitert, Kategorie-Labels laufen
      jetzt tatsächlich durch gettext (1.0.12).
- [ ] Alte Panels (privacy, power, media, …) auf
      panels::common-Helfer migrieren.
- [x] Energie-Panel-Absturz behoben (1.0.12): ungeprüfter
      Zugriff auf org.gnome.settings-daemon.plugins.power (gsd
      ist nicht Teil von Ragnar) tötete die App beim Öffnen;
      Zeilen sind jetzt schema-abgesichert. Die Profil-Umschaltung
      wurde vom powerprofilesctl-CLI auf die
      net.hadess.PowerProfiles-D-Bus-API umgestellt — funktioniert
      identisch mit power-profiles-daemon oder tuned-ppd
      (tuned-ppd in die ISO-performance.list.chroot aufgenommen,
      tuned bleibt die Tuning-Engine).
- [x] Software-Kachel startet ragnar-software (1.0.12);
      Drucker-Panel neu geschrieben — listet konfigurierte
      Warteschlangen mit Standard/Löschen, kein Hinzufügen
      (das gehört den Treibern); Standard-PDF-Drucker wird
      per CUPS-PDF automatisch angelegt, mit editierbaren
      Benutzer-Defaults (Papiergröße, Auflösung, PDF-Version).
- [x] Terminal-argv vereinfacht (1.0.12): Sonderfälle pro
      Emulator entfernt — gnome-terminal/kgx nutzen `--`,
      alles andere bekommt die portable `-e sh -c`-Form.
- [x] Bildschirmtastatur-/Screenreader-Zeilen (1.0.12,
      experimenteller Abschnitt in Barrierefreiheit): schreiben
      die echten a11y.applications-Flags und zeigen an, ob ein
      nutzendes Programm (squeekboard/onboard/orca) installiert
      ist — fehlende Werkzeuge verweisen auf Software, inklusive
      „Software öffnen"-Sprungknopf.
- [x] Panel & Desktop: org.lieraos.ragnar.*-Einstellungen sind
      exponiert — pro-Leiste-Optionen (Position, Autohide,
      Applet-Defaults) über das eigene Einstellungsfenster von
      ragnar-taskbar (bewusst kein doppeltes UI hier);
      notification-position im Benachrichtigungs-Panel ergänzt
      (1.0.12).

## Phase C — Normal vs. Erweitert

- [x] Erweitert-Modus (1.0.12): persistenter Schalter in der
      Kopfleiste, Zustand in
      `~/.config/ragnar-settings/settings.ini` (`[ui] advanced`);
      Kachel-Verstecken über `registry::catalog::ADVANCED_TILES`,
      Sektions-Guards über `crate::advanced::enabled()` in den
      Panels (GPU-Steuerung in Anzeige, rc-Bearbeitung in Eingabe,
      Gruppenverwaltung in Benutzer).
- [x] Suchfokus-Fix (1.0.12): Der Hub-Neuaufbau pro Tastenschlag
      ruft den Stale-Pointer-Grab-Workaround nicht mehr auf — das
      Feld behält den Tastaturfokus für durchgehendes Tippen.
- [x] Modulaufteilungen für das Zeilenbudget (1.0.12):
      display/canvas.rs → monitor_rect.rs (Rechtecke, Drag/Select,
      Thumbnails), registry/catalog.rs → entries.rs (Kachelliste).
- [x] Alle Clippy-Warnungen behoben; `cargo test`, `msgfmt --check`
      und Release-Build grün.
- [x] Compositor-Panel (1.0.12, Erweitert-Kachel): Fensterdekoration
      (Eckenradius, Schatten, Rahmen beibehalten, Titelleisten-Layout
      + -Schrift), Fensterabstand, Arbeitsflächen-Ränder,
      Arbeitsflächen-Anzahl + Hinweisdauer, Fokusverhalten,
      Einrastabstände, Oberer-Rand-Maximierung, Platzierungsrichtlinie,
      Tearing — live in die Compositor-Konfigurationsdatei
      geschrieben. Sektion „Kommt mit dem Ragnar-Compositor" listet
      gesperrte Zeilen (Unschärfe, Animationen, Kachel-Layouts,
      Gesten, Fenstergruppen, Scratchpad, Spiegelung).
- [x] Compositor-Capability-Schicht (1.0.12): `backend/compositor.rs`
      erkennt den laufenden Compositor (labwc / nativer Ragnar /
      unbekannt) und bildet den Funktionsumfang pro Feature ab;
      gesperrte Zeilen schalten sich selbst frei, sobald der native
      Compositor die Fähigkeit bietet — keine zweite UI.
- [x] Generischer rc.xml-Patcher (1.0.12): `panels/rcxml.rs` ersetzt
      den Input-only-`rc_edit`-Helfer — Block-Get/Set für beliebige
      Sektionen, verschachtelte Blöcke über Parent, Attribut-Lesen/
      Schreiben inkl. attribut-adressierter Elemente
      (`<font place="…">`), Auto-Anlage fehlender Sektionen/Elemente,
      `labwc -r`-Reload; Input- + Shortcuts-Panel migriert;
      Fixture-getestet.
- [x] Fensterregeln-Editor (1.0.12, Erweitert-Kachel,
      experimentell): Auflisten/Anlegen/Bearbeiten/Löschen der
      Fensterregeln des Compositors — Matcher (Anwendungs-ID,
      Titel, nur-erstes-Fenster), Fensterzustands-Aktionen
      (vorn/hinten, omnipräsent, maximiert, Vollbild, minimiert),
      Platzierung (Einrastkante, Zentrierung, Arbeitsfläche),
      Titelleisten-Überschreibung und Verhaltens-Flags.
      Round-Trip-Modell erhält nicht modellierte Attribute/
      Aktionen/Kommentare wortgetreu; Fixture-getestet.
- [x] Tastenkürzel-Editor v2 (1.0.12): jedes Compositor-Keybind
      ist editierbar — Tasten-Capture-Dialog plus
      WM-Aktions-Katalog (Befehl, Schließen/Minimieren,
      Maximieren, Vollbild, Einrasten, Arbeitsflächen-Wechsel/
      -Verschieben, Fensterwechsel, Menüs, Schreibtisch,
      Neuladen). Sitzungs-verwaltete Binds
      (managed="labwc-bridge") sind schreibgeschützt, weil die
      Bridge sie aus GSettings neu erzeugt.
- [x] Arbeitsflächen-Namen (1.0.12, Compositor-Panel):
      Namensfelder pro Arbeitsfläche schreiben
      <desktops><names> — leere Felder behalten den erzeugten
      „<Präfix> N"-Namen; die Feldanzahl folgt dem
      Arbeitsflächen-Zähler live.
- [x] Autostart & Umgebung (1.0.12, Erweitert-Kachel): Editoren
      für die Autostart-Datei der Sitzung (Programme beim
      Anmelden) und Umgebungsvariablen — direktes Bearbeiten
      unter Erhalt der Kommentare, sitzungs-verwaltete Variablen
      schreibgeschützt.
- [x] Eingabe-Tiefe (1.0.12): Doppelklick-Zeit, Tippen-und-
      Ziehen, Ziehsperre, Drei-Finger-Ziehen, Tipp-
      Tastenzuordnung, Klickmethode, Scroll-Methode,
      Scroll-Geschwindigkeit und pro-Fenster-Tastaturlayout
      im Eingabe-Panel ergänzt; ungültiger „behalten"-
      Numlock-Wert entfernt (der Compositor kennt nur
      an/aus). Touchpad-Zeilen bleiben ohne erkanntes
      Touchpad ausgeblendet.
- [x] Benachrichtigungs-Position (1.0.12, Benachrichtigungs-
      Panel): Bildschirmecken-Dropdown auf das
      notification-position-Enum des Panel-Schemas,
      schema-abgesichert — die Zeile erscheint nur, wo das
      Schema installiert ist.
- [x] Konfigurations-Sicherung & Wiederherstellung (1.0.12):
      Footer-Buttons öffnen Dateidialoge; der Export sammelt jede
      Quelle aus dem deklarativen Manifest in `src/backup/mod.rs`
      (~/.config-Dateien + -Verzeichnisse, Ganze-Schema-
      gsettings-Dumps inkl. aller zur Laufzeit gefundenen
      org.lieraos.ragnar.*-Schemas, Systemdateien) in ein
      .tar.gz mit Zeitstempel; die Wiederherstellung spielt
      Dateien + gsettings zurück und installiert Systemdateien
      über ein pkexec-Skript. Neue Quellen werden als eine
      Manifest-Zeile registriert — das Feature wächst mit.

## Phase D — App-Kacheln

- [x] Klang-Kachel → Liera Audio (`lieraaudio`).
- [x] Netzwerk- & Bluetooth-Kachel → Heimdall (`ragnar-heimdall`).
- [x] Software-Kachel → startet ragnar-software.

## Phase E — Übernahme

- [x] Debian-Paketierung — cargo-deb liefert Binary + .desktop + .mo.
- [x] In der ISO-Paketliste (packages.chroot-Staging).
- [x] ragnar-control-center aus der ISO entfernt — ragnar-settings
      ist die einzige Systemsteuerung.
- [ ] Weitere automatisierte Tests über ini/apt/terminal/search
      hinaus (display dbus_client-Parsing, Erweitert-Filterung).
