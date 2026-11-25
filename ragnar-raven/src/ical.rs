// Copyright (C) Jörg Schröder
//! iCalendar format helpers — the pieces of RFC 5545 Raven reads
//! and writes: reminder offsets (VALARM TRIGGER) and the DTSTART
//! datetime forms.

use gtk4::glib;

/// One appointment reminder offset, expressed as minutes before the
/// appointment start. Stored as a VALARM TRIGGER inside the VEVENT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reminder(pub i64);

impl Reminder {
    /// Standard reminder offsets offered in the editor (minutes).
    pub const PRESETS: &'static [i64] = &[2880, 1440, 360, 120, 60, 30, 15, 5, 0];

    /// ICS TRIGGER value, e.g. -PT48H / -PT30M / PT0S (at start time).
    pub(crate) fn to_trigger(self) -> String {
        let m = self.0;
        if m == 0 {
            "PT0S".into()
        } else if m % 60 == 0 {
            format!("-PT{}H", m / 60)
        } else {
            format!("-PT{}M", m)
        }
    }

    /// Parse an ICS TRIGGER value like -PT48H, -PT90M, -P1D.
    pub(crate) fn from_trigger(v: &str) -> Option<Self> {
        let neg = v.starts_with('-');
        let v = v.trim_start_matches('-').trim_start_matches('P');
        let v = v.strip_prefix('T').unwrap_or(v);
        let (num, unit) = v.split_at(v.len().checked_sub(1)?);
        let n: i64 = num.parse().ok()?;
        let minutes = match unit {
            "M" => n,
            "H" => n * 60,
            "S" => n / 60,
            _ => n * 1440, // -P1D arrives without a T prefix
        };
        Some(Self(if neg { minutes } else { -minutes }))
    }
}

/// Parse an ICS DTSTART into a local glib::DateTime. Accepts the
/// three RFC 5545 forms: floating local (`YYYYMMDDTHHMMSS`), UTC
/// (`…Z`, converted to local time) and date-only (`YYYYMMDD`,
/// all-day → local midnight). Anything else is rejected.
pub(crate) fn parse_dt(v: &str) -> Option<glib::DateTime> {
    let (v, utc) = match v.strip_suffix('Z') {
        Some(rest) => (rest, true),
        None => (v, false),
    };
    let y = v.get(0..4)?.parse().ok()?;
    let mo = v.get(4..6)?.parse().ok()?;
    let d = v.get(6..8)?.parse().ok()?;
    if v.len() == 8 {
        return glib::DateTime::from_local(y, mo, d, 0, 0, 0.0).ok();
    }
    if v.len() < 15 {
        return None;
    }
    let h = v[9..11].parse().ok()?;
    let mi = v[11..13].parse().ok()?;
    let s = v[13..15].parse().ok()?;
    let dt = if utc {
        glib::DateTime::from_utc(y, mo, d, h, mi, s).ok()?
    } else {
        glib::DateTime::from_local(y, mo, d, h, mi, s).ok()?
    };
    dt.to_local().ok()
}

pub(crate) fn fmt_dt(dt: &glib::DateTime) -> String {
    dt.format("%Y%m%dT%H%M%S").unwrap().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Floating local DTSTART stays at the same wall-clock time.
    #[test]
    fn parse_dt_floating_local() {
        let d = parse_dt("20260615T143000").unwrap();
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month(), 6);
        assert_eq!(d.day_of_month(), 15);
        assert_eq!(d.hour(), 14);
        assert_eq!(d.minute(), 30);
    }

    /// A `Z` suffix means UTC: the parsed instant must equal the UTC
    /// moment regardless of the local zone (unix timestamps are
    /// timezone-independent, so this is deterministic).
    #[test]
    fn parse_dt_utc_converts_to_local() {
        let parsed = parse_dt("20260615T120000Z").unwrap();
        let utc = glib::DateTime::from_utc(2026, 6, 15, 12, 0, 0.0).unwrap();
        assert_eq!(parsed.to_unix(), utc.to_unix());
    }

    /// Date-only DTSTART (VALUE=DATE, all-day events) lands on local
    /// midnight of that day instead of being dropped to "now".
    #[test]
    fn parse_dt_date_only() {
        let d = parse_dt("20261224").unwrap();
        assert_eq!(d.day_of_month(), 24);
        assert_eq!(d.month(), 12);
        assert_eq!(d.hour(), 0);
        assert_eq!(d.minute(), 0);
    }

    /// Garbage and truncated values are rejected, not guessed.
    #[test]
    fn parse_dt_rejects_bad_input() {
        assert!(parse_dt("").is_none());
        assert!(parse_dt("2026").is_none());
        assert!(parse_dt("20260615T1430").is_none()); // 12 chars, not 15
    }

    /// Every preset round-trips through ICS TRIGGER serialisation.
    #[test]
    fn reminder_trigger_roundtrip() {
        for &m in Reminder::PRESETS {
            let t = Reminder(m).to_trigger();
            assert_eq!(Reminder::from_trigger(&t), Some(Reminder(m)));
        }
    }

    /// Foreign-file shapes (-P1D without a T prefix) still parse.
    #[test]
    fn reminder_trigger_day_form() {
        assert_eq!(Reminder::from_trigger("-P1D"), Some(Reminder(1440)));
        assert_eq!(Reminder::from_trigger("-PT90M"), Some(Reminder(90)));
    }
}
