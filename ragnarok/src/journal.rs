// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Journald collector — runs `journalctl -p err -o json` and turns
// each record into an Entry. Priority <= err means real errors only
// (emerg/alert/crit/err), never warnings.

use crate::entry::Entry;
use std::process::Command;

/// Collect journald errors for the given boot offset.
/// `boot = Some(0)` current boot, `Some(-1)` previous, `None` all boots.
pub fn collect(boot: Option<i32>) -> Vec<Entry> {
    let mut cmd = Command::new("journalctl");
    if let Some(b) = boot {
        cmd.arg("-b").arg(b.to_string());
    }
    cmd.args(["-p", "err", "-o", "json", "--no-pager", "-q"]);
    let out = match cmd.output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&out)
        .lines()
        .filter_map(parse_line)
        .collect()
}

/// Parse one JSON record line into an Entry; malformed lines are skipped.
fn parse_line(line: &str) -> Option<Entry> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let field = |k: &str| v[k].as_str().unwrap_or("");
    let message = field("MESSAGE").to_string();
    if message.is_empty() {
        return None;
    }
    // Group by the most specific source name available: unit file,
    // syslog identifier, kernel transport, else generic fallback.
    let group = if field("_SYSTEMD_UNIT") != "" {
        field("_SYSTEMD_UNIT").to_string()
    } else if field("SYSLOG_IDENTIFIER") != "" {
        field("SYSLOG_IDENTIFIER").to_string()
    } else if field("_TRANSPORT") == "kernel" {
        "kernel".to_string()
    } else {
        field("_COMM").to_string()
    };
    Some(Entry {
        group: if group.is_empty() {
            "other".into()
        } else {
            group
        },
        timestamp: format_timestamp(field("__REALTIME_TIMESTAMP")),
        message,
    })
}

/// Turn the microsecond __REALTIME_TIMESTAMP into "YYYY-MM-DD HH:MM:SS".
fn format_timestamp(us: &str) -> String {
    let secs: i64 = match us.parse::<i64>() {
        Ok(u) => u / 1_000_000,
        Err(_) => return String::new(),
    };
    let t = secs + local_offset(secs);
    let (days, rem) = (t.div_euclid(86400), t.rem_euclid(86400));
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

/// Local UTC offset in seconds, read once from /etc/localtime via TZ.
/// Falls back to 0 (UTC) when the zone file cannot be read.
fn local_offset(secs: i64) -> i64 {
    // Ask libc for the offset rather than parsing tzfiles ourselves.
    extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const u8,
    }
    unsafe {
        let mut tm = std::mem::zeroed::<Tm>();
        if localtime_r(&secs, &mut tm).is_null() {
            0
        } else {
            tm.gmtoff
        }
    }
}

/// Days since epoch -> (year, month, day), Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
