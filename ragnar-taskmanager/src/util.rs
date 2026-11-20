// SPDX-License-Identifier: GPL-3.0-or-later
//! Tiny shared formatters used by several tabs.

/// kB → human readable size string.
pub fn kb_human(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

/// Linux task state letter → readable name.
pub fn state_name(c: char) -> &'static str {
    crate::i18n::msgid(match c {
        'R' => "Running",
        'S' => "Sleeping",
        'D' => "Disk sleep",
        'T' | 't' => "Stopped",
        'Z' => "Zombie",
        'I' => "Idle",
        _ => "Unknown",
    })
}
