//! Appointment store backed by iCalendar (.ics) files.
//!
//! Layout: one VCALENDAR file at
//! `$XDG_DATA_HOME/ragnar-raven/appointments.ics`
//! (~/.local/share/ragnar-raven/appointments.ics). Every appointment is
//! a VEVENT with SUMMARY, DTSTART (floating local time), UID and one
//! VALARM per reminder offset (TRIGGER:-PT…). VALARMs map 1:1 to the
//! CalDAV model, so a later Nextcloud/ownCloud sync can push these
//! files without any format conversion.

use gtk4::glib;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

/// One appointment reminder offset, expressed as minutes before the
/// appointment start. Stored as a VALARM TRIGGER inside the VEVENT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reminder(pub i64);

impl Reminder {
    /// Standard reminder offsets offered in the editor (minutes).
    pub const PRESETS: &'static [i64] = &[2880, 1440, 360, 120, 60, 30, 15, 5, 0];

    /// ICS TRIGGER value, e.g. -PT48H / -PT30M / PT0S (at start time).
    fn to_trigger(&self) -> String {
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
    fn from_trigger(v: &str) -> Option<Self> {
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

/// A single appointment (VEVENT).
#[derive(Debug, Clone)]
pub struct Appointment {
    pub uid: String,
    pub title: String,
    /// Start as local wall time formatted `YYYYMMDDTHHMMSS`.
    pub start: glib::DateTime,
    /// Reminder offsets, ascending (largest offset = earliest reminder
    /// first). Multiple offsets are allowed; they fire independently.
    pub reminders: BTreeSet<i64>,
}

impl Appointment {
    /// Next pending reminder for the reminder daemon (step 3).
    /// Local-time arithmetic is used deliberately: reminders are
    /// wall-clock relative.
    #[allow(dead_code)]
    pub fn next_reminder(&self) -> Option<glib::DateTime> {
        let now = glib::DateTime::now_local().ok()?;
        self.reminders
            .iter()
            .rev() // largest offset first — fires earliest
            .filter_map(|m| self.start.add(glib::TimeSpan::from_minutes(-*m)).ok())
            .find(|t| t > &now)
    }
}

/// Parse `YYYYMMDDTHHMMSS` (floating local) into a glib::DateTime.
fn parse_dt(v: &str) -> Option<glib::DateTime> {
    if v.len() < 15 {
        return None;
    }
    let y = v[0..4].parse().ok()?;
    let mo = v[4..6].parse().ok()?;
    let d = v[6..8].parse().ok()?;
    let h = v[9..11].parse().ok()?;
    let mi = v[11..13].parse().ok()?;
    let s = v[13..15].parse().ok()?;
    glib::DateTime::from_local(y, mo, d, h, mi, s).ok()
}

fn fmt_dt(dt: &glib::DateTime) -> String {
    dt.format("%Y%m%dT%H%M%S").unwrap().to_string()
}

/// Path of the calendar file; creates the parent dir on first use.
fn ics_path() -> PathBuf {
    let dir = glib::user_data_dir().join("ragnar-raven");
    let _ = fs::create_dir_all(&dir);
    dir.join("appointments.ics")
}

/// The shared appointment store. Cheap to clone (Rc inside); all UI
/// pieces hold the same instance and see each other's edits.
#[derive(Clone)]
pub struct Store {
    inner: Rc<RefCell<Vec<Appointment>>>,
}

impl Store {
    /// Load all VEVENTs from the .ics file. Missing file = empty store.
    pub fn load() -> Self {
        let mut items = Vec::new();
        if let Ok(text) = fs::read_to_string(ics_path()) {
            // ICS folds long lines (continuation starts with space).
            let text = text.replace("\r\n ", "").replace("\r\n", "\n");
            let mut cur: Option<Appointment> = None;
            for line in text.lines() {
                let line = line.trim_end();
                match line {
                    "BEGIN:VEVENT" => {
                        cur = Some(Appointment {
                            uid: String::new(),
                            title: String::new(),
                            start: glib::DateTime::now_local().unwrap(),
                            reminders: BTreeSet::new(),
                        });
                    }
                    "END:VEVENT" => {
                        if let Some(a) = cur.take() {
                            if !a.uid.is_empty() {
                                items.push(a);
                            }
                        }
                    }
                    _ => {
                        if let Some(a) = cur.as_mut() {
                            if let Some(v) = line.strip_prefix("UID:") {
                                a.uid = v.into();
                            } else if let Some(v) = line.strip_prefix("SUMMARY:") {
                                a.title = v.into();
                            } else if line.starts_with("DTSTART") {
                                if let Some(v) = line.rsplit(':').next() {
                                    if let Some(d) = parse_dt(v.trim_end_matches('Z')) {
                                        a.start = d;
                                    }
                                }
                            } else if let Some(v) = line.strip_prefix("TRIGGER:") {
                                if let Some(r) = Reminder::from_trigger(v) {
                                    a.reminders.insert(r.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        let s = Self {
            inner: Rc::new(RefCell::new(items)),
        };
        s.sort();
        s
    }

    fn sort(&self) {
        self.inner
            .borrow_mut()
            .sort_by(|a, b| a.start.cmp(&b.start));
    }

    /// Persist the whole store back to the .ics file.
    pub fn save(&self) {
        let mut out = String::from(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//LieraOS//Ragnar Raven//EN\r\n",
        );
        for a in self.inner.borrow().iter() {
            out.push_str("BEGIN:VEVENT\r\n");
            out.push_str(&format!("UID:{}\r\n", a.uid));
            out.push_str(&format!("SUMMARY:{}\r\n", a.title.replace(['\n', '\r'], " ")));
            out.push_str(&format!("DTSTART:{}\r\n", fmt_dt(&a.start)));
            for m in &a.reminders {
                out.push_str("BEGIN:VALARM\r\nACTION:DISPLAY\r\n");
                out.push_str(&format!("DESCRIPTION:{}\r\n", a.title));
                out.push_str(&format!("TRIGGER:{}\r\n", Reminder(*m).to_trigger()));
                out.push_str("END:VALARM\r\n");
            }
            out.push_str("END:VEVENT\r\n");
        }
        out.push_str("END:VCALENDAR\r\n");
        let _ = fs::write(ics_path(), out);
    }

    /// Insert a new appointment (fresh UID) and save.
    pub fn add(&self, title: &str, start: glib::DateTime, reminders: BTreeSet<i64>) {
        let uid = format!(
            "{}-{}@ragnar-raven",
            glib::DateTime::now_local().unwrap().to_unix(),
            glib::random_int()
        );
        self.inner.borrow_mut().push(Appointment {
            uid,
            title: title.into(),
            start,
            reminders,
        });
        self.sort();
        self.save();
    }

    /// Remove an appointment by UID and save.
    pub fn remove(&self, uid: &str) {
        self.inner.borrow_mut().retain(|a| a.uid != uid);
        self.save();
    }

    /// All appointments that start on the given calendar day.
    pub fn on_day(&self, day: &glib::DateTime) -> Vec<Appointment> {
        self.inner
            .borrow()
            .iter()
            .filter(|a| {
                a.start.format("%Y-%m-%d").ok() == day.format("%Y-%m-%d").ok()
            })
            .cloned()
            .collect()
    }

    /// Every stored appointment, chronological — used by the
    /// reminder loop.
    pub fn all(&self) -> Vec<Appointment> {
        self.inner.borrow().clone()
    }

    /// Days (as `YYYYMMDD` strings) that carry at least one
    /// appointment — used by the taskbar calendar markers (step 2).
    #[allow(dead_code)]
    pub fn marked_days(&self) -> std::collections::BTreeSet<String> {
        self.inner
            .borrow()
            .iter()
            .filter_map(|a| a.start.format("%Y%m%d").ok())
            .map(|g| g.to_string())
            .collect()
    }
}
