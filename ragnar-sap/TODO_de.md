# ragnar-sap — TODO

## Erledigt

- [x] Crate-Gerüst (Cargo.toml, i18n, .desktop, po/)
- [x] SANE-FFI: rohe Bindings für libsane.so.1 (kein dev-Paket nötig)
- [x] Safe-Wrapper: init/exit, typisierter SaneError,
      Geräteaufzählung mit Scanner-Filter
- [x] Option-Deskriptoren + typisiertes Setzen (INT/FIXED/BOOL/STRING)
- [x] Capability-Probing: DPI-Listen, Modi, ADF/Duplex
      
- [x] Scan-Schleife: PNM-Wrap
      (P6/P5/P4), 64-KiB-Reads, Fallback bei Start-Fehler/exotischem
      Format/leerem Ergebnis, kein Fallback bei get_parameters-Fehler
- [x] End-to-End gegen libsanes `test`-Backend verifiziert
- [x] Unit-Tests: Modus-Mapping, PNM-Header, scanimage-e2e

## Phase 1 — Backends

- [x] Device-Registry (id+type-Dedupe,
      Metadata-Defaults) + Discovery-Orchestrierung über
      SANE/CUPS/USB

- [ ] An echter Hardware verifizieren
- [x] scanimage-CLI-Fallback (exec ohne
      Shell, stderr->/dev/null, libusb-Auto-Detect-Retry)
- [ ] Mehrseiten-Scans (ADF-Batch: lesen bis NO_DOCS)
- [x] CUPS-FFI: Dest-Aufzählung mit Physik-Drucker-Filter
      + URI-Dedupe-Scoring,
      PPD-Capability-Probe (Resolution/PageSize/PrintQuality),
      Job-Submission via cupsPrintFile2
- [x] Gegen lokalen CUPS verifiziert (virtuelle PDF-Queue gefiltert)
- [ ] Job-Queue-Status-Polling für die UI-Warteschlange
- [x] Netzwerk-Discovery — SANEs escl/airscan-Backends decken
      Netzwerkscanner nativ ab; kein extra Avahi-Layer nötig
- [x] USB-sysfs-Probe (Klasse 0x06/0xFF
      akzeptieren, 0x0E ablehnen, MFP-Keyword-Regeln,
      vid:pid-Dedupe)

## Phase 2 — Bildpipeline

- [x] PNM-Dekode zu RGB8 (P4/P5/P6, eigener Parser — kein
      image-Crate nötig, Pipeline ist Buffer-nativ)
- [x] Auto-Crop (Content-Bounds-Erkennung)
- [x] Rotate 90/180/270
- [x] Echte Orientierungserkennung via tesseract OSD --psm 0 —
      tesseract OSD --psm 0
- [x] Leerseitenfilter (95% weiß @240)
- [x] Helligkeit/Kontrast/Sättigung + analyse()-Auto-Optimierung
      
- [x] Redaktions-Rechtecke (echte
      Pixelmaskierung, an Bounds geclippt)
- [x] Papierformat-Erkennung
- [x] --pipeline-test End-to-End + 16 Unit-Tests

## Phase 3 — GTK4-UI

- [ ] Dashboard: Geräteübersicht, Status-Ticker, Auto-Refresh
- [ ] Scan→Print-Fenster: Gerätewahl, DPI/Modus/Quelle,
      Vorschau mit Zoom/Pan, Profile, virtueller PDF-Drucker
- [ ] Nur-Drucken-Fenster: Dateiwahl, Vorschau, Kopien/Qualität
- [ ] Einstellungen: Sprache (de/en/fr/es), Auto-Refresh, Profile

## Phase 4 — Workflows

- [x] Temp-Storage (XOR-
      verschleierte Blobs, .exp-TTL-Sidecars, Secure-Overwrite,
      Purge) — Entropie via getrandom(2), keine Dateireads
- [x] Scan-Sessions (16-Hex-Ids,
      begin/update/load/finalize/abort)
- [x] ScanWorkflow + ScanAndPrintWorkflow + PrintWorkflow +
      WorkflowEventBus (Statemachine + Callbacks)
- [x] PDF-Verwaltung: Combine/Compress (Ghostscript-argv, kein
      Shell), Delete/Reorder/Rotate (pdftk), TIFF-Export
      (pdftoppm), OCR (tesseract), Seitenzahl (pdfinfo)
- [x] ECHTES createPdfFromImages via lopdf (Image-XObjects,
      FlateDecode) + echte Metadaten lesen/schreiben — ersetzt
      echte Implementierung,
      ohne Shell-Aufrufe
- [ ] E-Mail-Versand (SMTP via lettre)
- [ ] Workflow-Vorlagen („Scan to PDF", „Scan to PDF & E-mail")

## Phase 5 — Paketierung

- [ ] cargo-deb: .desktop, .mo-Assets, Depends prüfen
- [ ] ragnar-sap in Lieras packages.chroot aufnehmen
