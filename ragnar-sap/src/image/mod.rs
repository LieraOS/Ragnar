// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-sap — image pipeline: PNM decode, crop/rotate, deskew,
// blank-page filter, color ops, redaction, PaperFormatDetector.

pub mod color;
pub mod crop;
pub mod deskew;
pub mod empty_page;
pub mod paper;
pub mod pnm;
pub mod redact;
pub mod ui_crop;

