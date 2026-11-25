# ragnar-sap — TODO

## Done

- [x] Crate skeleton (Cargo.toml, i18n, .desktop, po/)
- [x] SANE FFI: raw bindings for libsane.so.1 (no dev package needed)
- [x] Safe wrapper: init/exit, typed SaneError, device enumeration
      with scanner filter
- [x] Option descriptors + typed set (INT/FIXED/BOOL/STRING)
- [x] Capability probing: DPI lists, modes, ADF/duplex
- [x] Scan loop: PNM wrap
      (P6/P5/P4), 64 KiB reads, fallback on start-fail/exotic
      format/empty, no fallback on get_parameters fail
- [x] Verified end-to-end against libsane `test` backend
- [x] Unit tests: mode mapping, PNM headers, scanimage e2e

## Phase 1 — backends

- [x] Device registry (id+type dedupe,
      metadata defaults) + discovery orchestration over
      SANE/CUPS/USB

- [ ] Verify against real hardware
- [x] scanimage CLI fallback (exec without
      shell, stderr->/dev/null, libusb auto-detect retry)
- [ ] Multi-page scans (ADF batch: keep reading until NO_DOCS)
- [x] CUPS FFI: dest enumeration with physical-printer
      filter + URI dedupe scoring,
      PPD capability probe (Resolution/PageSize/PrintQuality),
      job submission via cupsPrintFile2
- [x] Verified against local CUPS (virtual PDF queue filtered)
- [ ] Job queue state polling for the UI queue view
- [x] Network discovery — SANE's escl/airscan backends cover
      network scanners natively; no extra Avahi layer needed
- [x] USB sysfs probe (class 0x06/0xFF
      accept, 0x0E reject, MFP keyword rules, vid:pid dedupe)

## Phase 2 — image pipeline

- [x] PNM decode to RGB8 (P4/P5/P6, own parser — no image crate
      needed since the pipeline is buffer-native)
- [x] Auto-crop (content-bounds detection)
- [x] Rotate 90/180/270
- [x] Real orientation detection via tesseract OSD --psm 0 —
      tesseract OSD --psm 0
- [x] Blank-page filter (95% white @240)
- [x] Brightness/contrast/saturation + analyse() auto-optimize
      
- [x] Redaction rectangles (real pixel
      masking, clipped to bounds)
- [x] Paper format detection
- [x] --pipeline-test end-to-end + 16 unit tests

## Phase 3 — GTK4 UI

- [ ] Dashboard: device overview, status ticker, auto-refresh
- [ ] Scan→Print window: device pickers, DPI/mode/source,
      preview with zoom/pan, profiles, virtual PDF printer
- [ ] Print-only window: file picker, preview, copies/quality
- [ ] Preferences: language (de/en/fr/es), auto-refresh, profiles

## Phase 4 — workflows

- [x] Temp storage (XOR-obfuscated
      blobs, .exp TTL sidecars, secure overwrite, purge) — entropy
      via getrandom(2), not file reads
- [x] Scan sessions (16-hex ids,
      begin/update/load/finalize/abort)
- [x] ScanWorkflow + ScanAndPrintWorkflow + PrintWorkflow +
      WorkflowEventBus (state machine + callbacks)
- [x] PDF management: combine/compress (ghostscript argv, no
      shell), delete/reorder/rotate (pdftk), TIFF export
      (pdftoppm), OCR (tesseract), page count (pdfinfo)
- [x] REAL createPdfFromImages via lopdf (image XObjects,
      FlateDecode) + real metadata read/write — — real PDF writer
      with no shell calls
- [ ] E-mail sending (SMTP via lettre)
- [ ] Workflow templates ("Scan to PDF", "Scan to PDF & E-mail")

## Phase 5 — packaging

- [ ] cargo-deb: .desktop, .mo assets, Depends sanity check
- [ ] Ship ragnar-sap in Liera packages.chroot
