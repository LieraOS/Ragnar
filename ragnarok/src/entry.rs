// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Shared record for one collected error line.

/// One error entry from journald or a log file.
pub struct Entry {
    /// Group key shown in the report header (unit, identifier, or file).
    pub group: String,
    /// Human readable timestamp ("2026-11-23 04:12:01") or empty.
    pub timestamp: String,
    /// The error message text.
    pub message: String,
}
