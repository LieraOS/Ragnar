# ragnar-sap

**Ragnar SAP** — Scan- und Druck-Workflows für den Ragnar-Desktop,
in Rust mit GTK4 geschrieben: Geräteerkennung
über SANE/CUPS, Scan-to-Print-Workflows, PDF-Verwaltung und
Workflow-Automatisierung.

## Status

Frühe Entwicklung. Die SANE-FFI-Schicht (Init, Geräteerkennung,
Optionen, Capability-Probing, Single-Pass-Scan) ist implementiert
und gegen libsanes `test`-Backend verifiziert.

## Bauen

```sh
cargo build --release
```

Benötigt GTK4-Entwicklungsheader (`libgtk-4-dev`) und zur Laufzeit
`libsane1`. `libsane-dev` ist zum Bauen nicht nötig — die FFI-Schicht
linkt den stabilen Soname `libsane.so.1` direkt.

## Verifikation an echter Hardware

```sh
# Scanner auflisten + ausgelesene Fähigkeiten
./target/release/ragnar-sap --list
# eine Seite in eine PNM-Datei scannen
./target/release/ragnar-sap --scan-test <geraete-id> out.pnm
```

Ohne angeschlossenen Scanner funktioniert libsanes `test`-Backend:

```sh
mkdir -p /tmp/sane-cfg && echo test > /tmp/sane-cfg/dll.conf
SANE_CONFIG_DIR=/tmp/sane-cfg ./target/release/ragnar-sap --scan-test test:0 out.pnm
```

## Aufbau

    src/
      main.rs          Einstieg, GTK4-App + Backend-Test-Flags
      i18n.rs          gettext (deutsche msgid, po/en.po übersetzt)
      sane/            libsane-FFI
        sys.rs           rohe extern-Deklarationen (libsane.so.1)
        status.rs        SANE_Status -> typisierter SaneError
        init.rs          prozessweites sane_init/sane_exit
        device.rs        sane_get_devices + Scanner-Filter
        option.rs        Descriptor-Walk + typisiertes Option-Set/Get
        capability.rs    DPI-/Modus-/Quellen-Probing
        session.rs       open -> Optionen -> sane_start/read -> ScannedPage

## Lizenz

GPL-3.0-or-later. Copyright (C) Jörg Schröder.
