// Copyright (C) Jörg Schröder
//! Appointment store backed by iCalendar (.ics) files.
//!
//! Layout: one VCALENDAR file at
//! `$XDG_DATA_HOME/ragnar-raven/appointments.ics`
//! (~/.local/share/ragnar-raven/appointments.ics). Every appointment is
//! a VEVENT with SUMMARY, DTSTART (floating local time), UID and one
//! VALARM per reminder offset (TRIGGER:-PT…). VALARMs map 1:1 to the
//! CalDAV model, so a later Nextcloud/ownCloud sync can push these
//! files without any format conversion.

use crate::ical::{fmt_dt, parse_dt, Reminder};
use gtk4::glib;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A single appointment (VEVENT).
#[derive(Debug, Clone)]
pub struct Appointment {
    pub uid: String,
    pub title: String,
    /// Optional short note shown under the title (DESCRIPTION).
    pub desc: String,
    /// Optional per-appointment notification sound — an absolute
    /// path inside util::sounds_dir() (a copy, so it survives the
    /// original being moved). Empty = global notification sound.
    /// Stored as X-RAGNAR-SOUND in the VEVENT.
    pub sound: String,
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
            // RFC 5545: line endings are CRLF, but LF files exist in
            // the wild — normalise first, then unfold every
            // continuation line (a fold is a break + single space OR
            // horizontal tab).
            let text = text
                .replace("\r\n", "\n")
                .replace("\n ", "")
                .replace("\n\t", "");
            let mut cur: Option<Appointment> = None;
            // VALARM blocks carry their own DESCRIPTION/TRIGGER —
            // only the VEVENT-level ones belong to the appointment.
            let mut in_valarm = false;
            for line in text.lines() {
                let line = line.trim_end();
                match line {
                    "BEGIN:VEVENT" => {
                        cur = Some(Appointment {
                            uid: String::new(),
                            title: String::new(),
                            desc: String::new(),
                            sound: String::new(),
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
                    "BEGIN:VALARM" => in_valarm = true,
                    "END:VALARM" => in_valarm = false,
                    _ => {
                        if let Some(a) = cur.as_mut() {
                            if let Some(v) = line.strip_prefix("UID:") {
                                a.uid = v.into();
                            } else if let Some(v) = line.strip_prefix("SUMMARY:") {
                                a.title = v.into();
                            } else if !in_valarm {
                                if let Some(v) = line.strip_prefix("DESCRIPTION:") {
                                    a.desc = v.into();
                                } else if let Some(v) = line.strip_prefix("X-RAGNAR-SOUND:") {
                                    a.sound = v.into();
                                } else if line.starts_with("DTSTART") {
                                    if let Some(v) = line.rsplit(':').next() {
                                        if let Some(d) = parse_dt(v) {
                                            a.start = d;
                                        }
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
            out.push_str(&format!(
                "SUMMARY:{}\r\n",
                a.title.replace(['\n', '\r'], " ")
            ));
            out.push_str(&format!("DTSTART:{}\r\n", fmt_dt(&a.start)));
            if !a.desc.is_empty() {
                out.push_str(&format!(
                    "DESCRIPTION:{}\r\n",
                    a.desc.replace(['\n', '\r'], " ")
                ));
            }
            if !a.sound.is_empty() {
                out.push_str(&format!("X-RAGNAR-SOUND:{}\r\n", a.sound));
            }
            for m in &a.reminders {
                out.push_str("BEGIN:VALARM\r\nACTION:DISPLAY\r\n");
                out.push_str(&format!("DESCRIPTION:{}\r\n", a.title));
                out.push_str(&format!("TRIGGER:{}\r\n", Reminder(*m).to_trigger()));
                out.push_str("END:VALARM\r\n");
            }
            out.push_str("END:VEVENT\r\n");
        }
        out.push_str("END:VCALENDAR\r\n");
        crate::util::write_atomic(&ics_path(), &out);
    }

    /// Resolve a caller-picked sound file into the stored path:
    /// files outside the sounds dir get copied in, paths already
    /// inside pass through. None/empty clears the custom sound.
    fn resolve_sound(uid: &str, sound: Option<&Path>) -> String {
        match sound {
            Some(p) => crate::util::copy_sound(uid, p)
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default(),
            None => String::new(),
        }
    }

    /// Insert a new appointment (fresh UID) and save. `sound` is a
    /// user-picked audio file — copied into the store's sounds dir.
    pub fn add(
        &self,
        title: &str,
        desc: &str,
        start: glib::DateTime,
        reminders: BTreeSet<i64>,
        sound: Option<&Path>,
    ) {
        let uid = format!(
            "{}-{}@ragnar-raven",
            glib::DateTime::now_local().unwrap().to_unix(),
            glib::random_int()
        );
        self.inner.borrow_mut().push(Appointment {
            sound: Self::resolve_sound(&uid, sound),
            uid,
            title: title.into(),
            desc: desc.into(),
            start,
            reminders,
        });
        self.sort();
        self.save();
    }

    /// Update the appointment with `uid` in place (title, start,
    /// reminder offsets) and save. Keeping the UID matters: the
    /// reminder daemon's fired-dedup and any external .ics tooling
    /// track appointments by it. Unknown uids are ignored — the
    /// entry may have been deleted since the list was rendered.
    /// A replaced sound copy is cleaned out of the sounds dir.
    pub fn update(
        &self,
        uid: &str,
        title: &str,
        desc: &str,
        start: glib::DateTime,
        reminders: BTreeSet<i64>,
        sound: Option<&Path>,
    ) {
        // A replaced sound copy must not linger in the sounds dir.
        let stale = {
            let mut items = self.inner.borrow_mut();
            let Some(a) = items.iter_mut().find(|a| a.uid == uid) else {
                return;
            };
            a.title = title.into();
            a.desc = desc.into();
            a.start = start;
            a.reminders = reminders;
            let old = std::mem::replace(&mut a.sound, Self::resolve_sound(uid, sound));
            if !old.is_empty() && old != a.sound {
                Some(old)
            } else {
                None
            }
        };
        if let Some(old) = stale {
            crate::util::drop_sound(Path::new(&old));
        }
        self.sort();
        self.save();
    }

    /// Remove an appointment by UID and save — its sound copy goes
    /// with it (files inside the sounds dir are ours to clean).
    pub fn remove(&self, uid: &str) {
        let mut gone_sound = String::new();
        self.inner.borrow_mut().retain(|a| {
            let keep = a.uid != uid;
            if !keep {
                gone_sound = a.sound.clone();
            }
            keep
        });
        if !gone_sound.is_empty() {
            crate::util::drop_sound(Path::new(&gone_sound));
        }
        self.save();
    }

    /// All appointments that start on the given calendar day.
    pub fn on_day(&self, day: &glib::DateTime) -> Vec<Appointment> {
        self.inner
            .borrow()
            .iter()
            .filter(|a| a.start.format("%Y-%m-%d").ok() == day.format("%Y-%m-%d").ok())
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
