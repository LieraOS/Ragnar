# ragnar-sap

**Ragnar SAP** — scan and print workflows for the Ragnar desktop,
written in Rust with GTK4: device
discovery via SANE/CUPS, scan-to-print workflows, PDF
management and workflow automation.

## Status

Early development. The SANE FFI layer (init, discovery, option
control, capability probing, single-pass scan) is implemented and
verified against libsane's `test` backend.

## Building

```sh
cargo build --release
```

Requires GTK4 development headers (`libgtk-4-dev`) and `libsane1`
at runtime. Build-time needs no `libsane-dev` — the FFI layer
links the stable soname `libsane.so.1` directly.

## Verification on real hardware

```sh
# enumerate scanners + probed capabilities
./target/release/ragnar-sap --list
# scan one page to a PNM file
./target/release/ragnar-sap --scan-test <device-id> out.pnm
```

With no physical scanner attached, libsane's `test` backend works:

```sh
mkdir -p /tmp/sane-cfg && echo test > /tmp/sane-cfg/dll.conf
SANE_CONFIG_DIR=/tmp/sane-cfg ./target/release/ragnar-sap --scan-test test:0 out.pnm
```

## Layout

    src/
      main.rs          entry point, GTK4 app + backend test flags
      i18n.rs          gettext (German msgid, po/en.po translates)
      sane/            libsane FFI
        sys.rs           raw extern declarations (libsane.so.1)
        status.rs        SANE_Status -> typed SaneError
        init.rs          process-wide sane_init/sane_exit
        device.rs        sane_get_devices + scanner filter
        option.rs        descriptor walk + typed option set/get
        capability.rs    DPI/mode/source probing
        session.rs       open -> options -> sane_start/read -> ScannedPage

## License

GPL-3.0-or-later. Copyright (C) Jörg Schröder.
