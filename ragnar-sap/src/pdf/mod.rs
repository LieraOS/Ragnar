// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — PDF management: ghostscript/pdftk/poppler/tesseract
// tool wrappers plus real lopdf-based creation and metadata.

// API surface for the PDF-management UI phase — not yet consumed
// from main.rs, so dead code is allowed module-wide.
#[allow(dead_code)]
pub mod create;
#[allow(dead_code)]
pub mod manager;
#[allow(dead_code)]
pub mod metadata;
#[allow(dead_code)]
mod tools;
