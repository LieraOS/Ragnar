# Ragnar Edit

Native Wayland text editor for the Ragnar desktop — the Rust/GTK4
successor to the Qt6 LieraEdit.

Copyright (C) 2026 Jörg Schröder — GPL-3.0-or-later.

## Features

- Menu bar (File / Edit / View / Settings / Help), document tabs
  with dirty indicators, status bar (Line / Column)
- Syntax highlighting for C/C++, Python, JS/TS, JSON, XML, HTML,
  Bash, CMake, Dockerfile, YAML, CSS, Markdown, INI, Rust, PHP, SQL
- Find & Replace with match-case, whole-word and search across all tabs
- Undo/redo, cut/copy/paste, zoom, printing
- Chunked background file loading, binary detection
- RTF → plain-text import, PDF read-only text extraction
- Settings: dark theme, font, word wrap, auto-save interval, language
- English/German UI via gettext

## Build

```bash
cargo build --release
```

Debian package: staged in `pkg/ragnar-edit` (`dpkg-deb --build`).
