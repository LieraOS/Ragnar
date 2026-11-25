# Ragnar Settings — die LieraOS-Systemsteuerung

Ragnar Settings ist das native Kontrollzentrum des Ragnar-Desktops,
geschrieben in **Rust + GTK4**. Das Ziel ist *Vertrautheit, nicht
Imitation*: Die Bedienung soll sich für Umsteiger von Windows sofort
selbstverständlich anfühlen — eine kategorisierte Übersicht, in der
jeder Bereich klar beschriftet und mit einem Klick erreichbar ist —
und bleibt dabei eine eigenständige LieraOS-Anwendung. Jeder Bereich
ist entweder ein eingebettetes natives Panel oder startet eine
bestehende LieraOS-Anwendung.

## Status

Voll funktionsfähige Systemsteuerung: kategorisierter Hub mit
Live-Suche, ~20 eingebettete Panels (Anzeige, Erscheinungsbild,
Klänge, Aktualisierungen, Benutzer, Standardanwendungen,
Wechseldatenträger, Nachtlicht, Barrierefreiheit, …) plus Kacheln,
die andere LieraOS-Werkzeuge starten. Zwei Zielgruppen-Ebenen:

- **Normalansicht** (Standard) — alles, was ein typischer
  Desktop-Benutzer erwartet: Anzeige, Hintergrundbild, Sprache,
  Panel & Desktop, Datum/Uhrzeit, Standardanwendungen,
  Aktualisierungen, Treiber.
- **Erweiterte Einstellungen** — ein persistenter Schalter in der
  Kopfleiste neben dem Suchfeld. Aktiviert er, öffnet sich die volle
  Einstelltiefe, die sonst Konfigurationsdateien erfordern würde:
  die GPU-Steuerung in der Anzeige (Power-Limit, fester Takt,
  Lüfterdrehzahl), Low-Level-Eingabekonfiguration, Gruppenverwaltung
  in Benutzer, sowie alle künftigen Kacheln aus
  `registry::catalog::ADVANCED_TILES`.

Im Hub-Footer sitzen **Konfiguration sichern / wiederherstellen**:
Der Export packt alle Konfigurationsquellen (Konfigurationsdateien,
gsettings-Dumps, Systemdateien) in ein einzelnes, vom Benutzer
gewähltes `.tar.gz` mit Zeitstempel; die Wiederherstellung entpackt
es und spielt alles zurück (Systemdateien über einen pkexec-Aufruf).
Die komplette Quellliste liegt als deklaratives Manifest in
`src/backup/mod.rs` — neue Einstellungen werden dort einmal
registriert.

Der Zustand liegt in `~/.config/ragnar-settings/settings.ini`
(`[ui] advanced=true|false`); Kacheln und abgesicherte
Panel-Sektionen prüfen ihn bei jedem Aufbau, sodass ein Umschalten
beim nächsten Hub-Aufbau bzw. Panel-Öffnen wirkt.

## Architektur

- **Hub-Fenster** — `GtkStack` + `ToolbarView`; die Hub-Seite ist ein
  kategorisiertes Kachelraster mit Suchfeld und Erweitert-Schalter in
  der Kopfleiste sowie Sichern/Wiederherstellen-Knöpfen in einer
  fixen Fußzeile. Kacheln legen Panel-Seiten auf den Stack; ein
  Zurück-Knopf führt zum Hub.
- **Registry-Katalog** (`src/registry/`) — jeder Bereich ist ein
  `Entry` (id, Titel, Untertitel, Icon, Kategorie, Aktion,
  Voraussetzung, Schlüsselwörter). `catalog.rs` enthält Typen und
  Zugriffsfunktionen, `entries.rs` die Kachelliste. Eine Kachel
  bettet entweder ein natives Panel ein (`Action::Panel`) oder
  startet ein externes Programm (`Action::Launch`). Neue Bereiche
  sind ein Eintrag — keine Hub-Änderungen nötig.
- **Verfügbarkeitsprüfungen** (`src/backend/`) — Einträge deklarieren
  ihre Voraussetzungen (`Always`, `Program`, `Schema`); nicht
  erfüllbare Kacheln werden ausgeblendet, sodass dieselbe Binary auf
  einem Entwicklungsrechner ohne Ragnar läuft und in einer
  Ragnar-Sitzung alles zeigt.
- **Compositor-Fähigkeiten** (`src/backend/compositor.rs`) — erkennt
  den laufenden Compositor (heute labwc, später der native
  Ragnar-Compositor) und bildet ab, was er kann. Tiefe
  Einstellungszeilen deklarieren die benötigte Fähigkeit:
  unterstützte Regler bleiben editierbar, alles andere wird gesperrt
  mit „Erfordert den Ragnar-Compositor.“ angezeigt — und schaltet
  sich selbst frei, sobald der native Compositor läuft. Die Karte
  ist die einzige Stelle, die je aktualisiert werden muss.
- **rc.xml-Patcher** (`src/panels/rcxml.rs`) — Texteditor für die
  Compositor-Konfigurationsdatei. Ersetzt vorhandene Elemente an
  Ort und Stelle, hängt fehlende Schlüssel in vorhandene Sektionen
  an, legt fehlende Top-Level-Sektionen vor `</labwc_config>` an und
  schreibt Attribute (auch auf attribut-adressierten Elementen wie
  `<font place="…">`). Benutzerverwaltete Inhalte (Tastenkürzel,
  Fensterregeln, Kommentare) bleiben unangetastet; `labwc -r`
  wendet Änderungen live an.
- **Native Panels** (`src/panels/`) für Ragnar-spezifische
  Einstellungen:
  - *Anzeige / Personalisierung* — Hintergrundbild, Icon-Design,
    Auflösung (wlr-output-management unter labwc), GPU-Steuerung
    (erweitert)
  - *Klänge* — Klangthemen installieren und wählen, Zuordnung pro
    Ereignis
  - *Panel & Desktop* — Applets, Raven, Autostart, Standard-Apps
  - *Wechseldatenträger* — Automount-Schalter
    (`org.lieraos.ragnar.automount enabled`, live von
    ragnar-automount konsumiert) plus Live-Liste eingehängter
    Wechseldatenträger mit Aushängen/Auswerfen-Knöpfen
  - *Compositor* (erweitert) — Fensterdekoration (Eckenradius,
    Fensterschatten, Rahmen beibehalten, Titelleisten-Layout/-Schrift),
    Fensterabstand und Arbeitsflächen-Ränder, Anzahl der
    Arbeitsflächen + Hinweisdauer, Fokusverhalten (Fokus folgt Maus,
    Anheben, Verzögerung), Einrastabstände, Oberer-Rand-Maximierung,
    Platzierung neuer Fenster und Tearing — alles live in die
    Compositor-Konfiguration geschrieben. Inklusive Namen pro
    Arbeitsfläche für den Wechsler (positionale
    <desktops><names>-Einträge). Eine Sektion „Kommt mit
    dem Ragnar-Compositor“ listet gesperrte Funktionen
    (Hintergrundunschärfe, Animationen, Kachel-Layouts, Gesten,
    Fenstergruppen, Scratchpad, Spiegelung), die sich automatisch
    freischalten, sobald der Compositor sie bereitstellt.
  - *Fensterregeln* (erweitert, experimentell) — vollständiger
    Editor für die Fensterregeln des Compositors: Abgleich über
    Anwendungs-ID oder Titel (Globs), danach pro Regel
    Fensterzustand (immer vorn/hinten, alle Arbeitsflächen,
    maximiert/Vollbild/minimiert), Platzierung (Einrastkante,
    Zentrierung, Ziel-Arbeitsfläche) und Verhalten
    (Titelleisten-Überschreibung, Taskleisten-/Wechsler-Ausblendung,
    Fokusblockade, Positionssperre). Attribute und Aktionen, die
    die Oberfläche nicht modelliert, bleiben unverändert
    erhalten; Regeln werden live gespeichert.
  - *Tastenkürzel* — vollständiger Keybind-Editor: Kombination
    aufnehmen und an jede Compositor-Aktion binden — Befehl
    ausführen, Fenster schließen/minimieren/maximieren/Vollbild,
    an Kanten einrasten, Arbeitsflächen wechseln/verschieben,
    Fensterwechsel, Menüs, Schreibtisch, Neuladen. Sitzungs-
    verwaltete Kürzel sind schreibgeschützt.
  - *Autostart & Umgebung* (erweitert) — Autostart-Datei der
    Sitzung (Programme beim Anmelden) und Umgebungsvariablen,
    direkt editierbar unter Erhalt der Kommentare; von der
    Sitzung verwaltete Variablen (Tastatur, Zeiger, Locale)
    sind schreibgeschützt.
    (gio::VolumeMonitor)
- **Gestartete Anwendungen** — Kacheln, die bestehende
  LieraOS-Werkzeuge öffnen:
  - *Klang* → **LieraSound** (Lautstärke, Geräte, Effekte)
  - *Netzwerk & Bluetooth* → **Heimdall** (in Entwicklung)
  - *Software* → Übersicht installierter Anwendungen →
    **Liera Software Center**

## Übersetzungen

Englisch ist die Quellsprache; alle sichtbaren Texte laufen über
`gettext()` (bzw. die `msgid()`-Markierung für Strings, die erst zur
Anzeigezeit übersetzt werden). Ein vollständiger deutscher Katalog
liegt in `po/de.po`; `build.rs` kompiliert jedes `po/*.po` per
`msgfmt` nach `target/locale/<lang>/LC_MESSAGES/`, und das
Debian-Paket installiert sie unter `/usr/share/locale/`. `LOCALEDIR`
überschreibt den Suchpfad für Entwicklungsläufe.

Kataloge nach Änderungen an sichtbaren Texten aktualisieren:

```sh
xgettext --language=Rust --keyword=gettext --keyword=msgid \
    --from-code=UTF-8 --package-name=ragnar-settings \
    --package-version=<ver> -o po/ragnar-settings.pot \
    src/*.rs src/*/*.rs src/*/*/*.rs
msgmerge -U po/de.po po/ragnar-settings.pot
```

## Entwicklung

```sh
cargo build        # bauen (kompiliert auch po/*.po -> .mo)
cargo test         # Unit-Tests (Registry-Suche, INI, apt, Terminal)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run          # lokal ausführen
LANGUAGE=de LOCALEDIR=target/locale cargo run   # auf Deutsch starten
```

## Entwicklungshinweise

- GTK 4.18 stürzt mit einem Stack-Overflow in `real_choose_icon` ab,
  wenn die Icon-Theme-Fallback-Kette defekt ist (z. B. ein
  unvollständiges lokales
  `~/.local/share/icons/hicolor/index.theme`, das das Systemtheme
  überdeckt). Eine nackte `GtkHeaderBar` reproduziert es
  systemweit. Stirbt die Anwendung sofort auf einem
  Entwicklungsrechner, zuerst die lokalen Icon-Themes prüfen — das
  ist ein Host-Defekt, kein Anwendungsfehler.

## Letzte Änderungen

- 1.0.12 — Compositor-Panel (erweitert): Fensterdekoration,
  Abstände/Ränder, Arbeitsflächen, Fokus, Snapping, Platzierung und
  Tearing — live in die Compositor-Konfiguration geschrieben durch
  einen generischen rc.xml-Patcher (`src/panels/rcxml.rs`, ersetzt
  den bisherigen Input-Editor; Input- + Shortcuts-Panel migriert).
  Gesperrte Zeilen für compositor-abhängige Funktionen werden über
  die neue Capability-Karte (`src/backend/compositor.rs`) automatisch
  freigeschaltet, sobald der native Ragnar-Compositor läuft. Patcher
  durch einen Fixture-Test abgedeckt. Ebenfalls in 1.0.12:
  Fensterregeln-Editor (erweitert, experimentell — Matcher, Zustand,
  Platzierung, Dekoration; verlustfreier Round-Trip) und der
  Tastenkürzel-Editor v2 (vollständige Keybind-Bearbeitung mit
  WM-Aktions-Katalog; sitzungs-verwaltete Kürzel schreibgeschützt),
  Eingabe-Tiefe (Doppelklick-Zeit, Tippen-und-Ziehen/Ziehsperre,
  Klick- & Scroll-Methoden, pro-Fenster-Tastaturlayout) und ein
  Bildschirmecken-Dropdown für Benachrichtigungen. Außerdem:
  Die Software-Kachel startet ragnar-software, und das
  Drucker-Panel verwaltet konfigurierte CUPS-Warteschlangen
  (Standard/Löschen, kein Hinzufügen) plus den
  Standard-PDF-Drucker — automatisch per CUPS-PDF angelegt,
  mit editierbaren Defaults für Papiergröße, Auflösung und
  PDF-Version. Barrierefreiheit bekam einen experimentellen
  Abschnitt mit Bildschirmtastatur- und Screenreader-Schaltern,
  die die standardmäßigen a11y.applications-Flags schreiben
  (jede Zeile meldet, ob ein nutzendes Programm wie
  squeekboard oder orca installiert ist; fehlende Werkzeuge
  verweisen auf Software, mit „Software öffnen"-Sprungknopf).
  Ein harter Absturz im Energie-Panel wurde behoben
  (ungeprüfter Zugriff auf das gsd-power-Schema, das Ragnar
  nicht ausliefert); Energieprofile nutzen jetzt direkt die
  net.hadess.PowerProfiles-D-Bus-API und funktionieren damit
  sowohl mit power-profiles-daemon als auch mit tuned-ppd.
  Standardanwendungen wurde um Kalender- und
  Archiv-Kategorien erweitert.
- 1.0.12 — Erweiterte Einstellungen (Schalter in der Kopfleiste,
  persistiert in `~/.config/ragnar-settings/settings.ini`);
  Konfigurations-Sicherung & Wiederherstellung im Hub-Footer
  (ein `.tar.gz`, manifestgesteuert in `src/backup/`); deutscher
  Katalog (`po/de.po`, ~510 Strings), von `build.rs` kompiliert und
  im Paket ausgeliefert; Suchfeld behält den Fokus während der
  Ergebnisaktualisierung; `display/canvas.rs` nach `monitor_rect.rs`
  und `catalog.rs` nach `entries.rs` aufgeteilt; alle
  Clippy-Warnungen behoben; `eprintln!` durchgängig durch `tracing`
  ersetzt.

## Paketierung

Wird als Debian-Binärpaket `ragnar-settings` ausgeliefert
(cargo-deb): Binary, `.desktop`-Datei und kompilierte
gettext-Kataloge unter `/usr/share/locale/<lang>/LC_MESSAGES/`.
