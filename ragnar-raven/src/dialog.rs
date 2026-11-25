// Copyright (C) Jörg Schröder
//! Appointment editor dialog: title, start time and an arbitrary set
//! of reminder offsets. Opened for the day the user picked — either
//! by double-clicking a calendar day or via the right-click menu.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use crate::i18n::tr;
use crate::ical::Reminder;
use crate::store::Store;

/// Label shown on the sound picker — the picked file's basename or
/// "Standard" when the appointment uses the global sound.
fn sound_label(p: &Option<PathBuf>) -> String {
    p.as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| tr("Standard"))
}

/// Human-readable label for a reminder offset given in minutes.
fn offset_label(minutes: i64) -> String {
    match minutes {
        0 => tr("Zum Terminzeitpunkt"),
        m if m % 1440 == 0 => format!("{} {}", m / 1440, tr("Tage vorher")),
        m if m % 60 == 0 => format!("{} {}", m / 60, tr("Stunden vorher")),
        m => format!("{} {}", m, tr("Minuten vorher")),
    }
}

/// Show the appointment editor anchored to `parent`, pre-filled
/// for the calendar day `day`. With `existing` the dialog edits
/// that appointment (title/time/reminders; the UID survives so the
/// reminder dedup stays attached), without it a new one is added.
/// On save the appointment goes straight into the shared store
/// (which persists to the .ics file). Returns the editor window
/// so the caller can react to it being closed.
pub fn open_editor(
    parent: &gtk4::Widget,
    day: &glib::DateTime,
    store: &Store,
    existing: Option<&crate::store::Appointment>,
) -> gtk4::Window {
    let win = gtk4::Window::builder()
        .title(if existing.is_some() {
            tr("Termin bearbeiten")
        } else {
            tr("Neuer Termin")
        })
        .modal(true)
        .default_width(340)
        .build();
    if let Some(toplevel) = parent.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&toplevel));
    }

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);

    // Day headline so the user always sees which day is being edited.
    let headline = gtk4::Label::new(None);
    headline.set_markup(&format!("<b>{}</b>", day.format("%A, %d. %B %Y").unwrap()));
    body.append(&headline);

    // Title — pre-filled when editing.
    let title = gtk4::Entry::builder()
        .placeholder_text(tr("Titel des Termins"))
        .build();
    if let Some(a) = existing {
        title.set_text(&a.title);
    }
    body.append(&title);

    // Optional short note shown under the title — DESCRIPTION in
    // the VEVENT.
    let desc = gtk4::Entry::builder()
        .placeholder_text(tr("Kurze Info zum Termin"))
        .build();
    if let Some(a) = existing {
        desc.set_text(&a.desc);
    }
    body.append(&desc);

    // Start time as two spin buttons (hour / minute), pre-filled
    // with the appointment's time when editing and the current
    // time when adding — rendered zero-padded (08:00).
    let initial = existing
        .map(|a| a.start.clone())
        .unwrap_or_else(|| glib::DateTime::now_local().unwrap());
    let time_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    time_row.append(&gtk4::Label::new(Some(&tr("Uhrzeit"))));
    let hour = gtk4::SpinButton::with_range(0.0, 23.0, 1.0);
    hour.set_numeric(true);
    hour.set_value(initial.hour() as f64);
    hour.connect_output(|sb| {
        sb.set_text(&format!("{:02}", sb.value() as i32));
        glib::Propagation::Stop
    });
    let colon = gtk4::Label::new(Some(":"));
    let minute = gtk4::SpinButton::with_range(0.0, 59.0, 1.0);
    minute.set_numeric(true);
    minute.set_value(initial.minute() as f64);
    minute.connect_output(|sb| {
        sb.set_text(&format!("{:02}", sb.value() as i32));
        glib::Propagation::Stop
    });
    time_row.append(&hour);
    time_row.append(&colon);
    time_row.append(&minute);
    body.append(&time_row);

    // Optional per-appointment notification sound — picked via
    // FileDialog, stored per appointment (X-RAGNAR-SOUND in the
    // .ics; the store copies the file into its sounds dir on save
    // so the original may move or disappear).
    let sound_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(
        existing
            .filter(|a| !a.sound.is_empty())
            .map(|a| PathBuf::from(&a.sound)),
    ));
    let sound_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    sound_row.append(&gtk4::Label::new(Some(&tr("Ton"))));
    let pick_lbl = gtk4::Label::new(Some(&sound_label(&sound_path.borrow())));
    pick_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    pick_lbl.set_hexpand(true);
    pick_lbl.set_halign(gtk4::Align::Start);
    let pick = gtk4::Button::with_label(&tr("Wählen …"));
    let clear = gtk4::Button::from_icon_name("edit-clear-symbolic");
    clear.set_has_frame(false);
    clear.set_tooltip_text(Some(&tr("Standard wiederherstellen")));
    clear.set_sensitive(sound_path.borrow().is_some());
    sound_row.append(&pick_lbl);
    sound_row.append(&pick);
    sound_row.append(&clear);
    body.append(&sound_row);

    pick.connect_clicked({
        let (sp, lbl, w) = (sound_path.clone(), pick_lbl.clone(), win.clone());
        let clear = clear.clone();
        move |_| {
            let dlg = gtk4::FileDialog::new();
            let filter = gtk4::FileFilter::new();
            filter.add_mime_type("audio/*");
            filter.set_name(Some(&tr("Audiodateien")));
            let filters = gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            dlg.set_filters(Some(&filters));
            dlg.open(Some(&w), gio::Cancellable::NONE, {
                let (sp, lbl, clear) = (sp.clone(), lbl.clone(), clear.clone());
                move |res| {
                    if let Ok(f) = res {
                        if let Some(p) = f.path() {
                            *sp.borrow_mut() = Some(p);
                            lbl.set_label(&sound_label(&sp.borrow()));
                            clear.set_sensitive(true);
                        }
                    }
                }
            });
        }
    });
    clear.connect_clicked({
        let (sp, lbl) = (sound_path.clone(), pick_lbl.clone());
        move |b| {
            *sp.borrow_mut() = None;
            lbl.set_label(&sound_label(&sp.borrow()));
            b.set_sensitive(false);
        }
    });

    // Reminder offsets: check boxes for the presets plus any
    // non-preset offsets the appointment already carries (foreign
    // .ics or an earlier custom value must stay visible — what you
    // see is what gets saved). Multiple entries fire independently.
    let rem_head = gtk4::Label::new(Some(&tr("Erinnern")));
    rem_head.set_halign(gtk4::Align::Start);
    body.append(&rem_head);
    let mut offsets: BTreeSet<i64> = Reminder::PRESETS.iter().copied().collect();
    if let Some(a) = existing {
        offsets.extend(a.reminders.iter().copied());
    }
    let mut checks = Vec::new();
    for m in offsets.iter().rev() {
        let cb = gtk4::CheckButton::with_label(&offset_label(*m));
        if let Some(a) = existing {
            cb.set_active(a.reminders.contains(m));
        }
        body.append(&cb);
        checks.push((cb, *m));
    }
    let custom_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let custom_cb = gtk4::CheckButton::new();
    let custom_min = gtk4::SpinButton::with_range(1.0, 43200.0, 5.0);
    custom_min.set_value(90.0);
    custom_min.set_sensitive(false);
    custom_cb.connect_toggled({
        let cm = custom_min.clone();
        move |b| cm.set_sensitive(b.is_active())
    });
    custom_row.append(&custom_cb);
    custom_row.append(&custom_min);
    custom_row.append(&gtk4::Label::new(Some(&tr("Minuten vorher"))));
    body.append(&custom_row);

    // Actions — kept OUTSIDE the scrolled area so they are always
    // reachable no matter how tall the field list gets.
    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    buttons.set_halign(gtk4::Align::End);
    buttons.set_margin_top(10);
    buttons.set_margin_bottom(16);
    buttons.set_margin_start(16);
    buttons.set_margin_end(16);
    let cancel = gtk4::Button::with_label(&tr("Abbrechen"));
    let save = gtk4::Button::with_label(&tr("Speichern"));
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    save.connect_clicked({
        let (w, st, d) = (win.clone(), store.clone(), day.clone());
        let existing_uid = existing.map(|a| a.uid.clone());
        move |_| {
            let text = title.text().trim().to_string();
            if text.is_empty() {
                return;
            }
            let desc_text = desc.text().trim().to_string();
            let mut rems = BTreeSet::new();
            for (cb, m) in &checks {
                if cb.is_active() {
                    rems.insert(*m);
                }
            }
            if custom_cb.is_active() {
                rems.insert(custom_min.value() as i64);
            }
            let start = glib::DateTime::from_local(
                d.year(),
                d.month(),
                d.day_of_month(),
                hour.value() as i32,
                minute.value() as i32,
                0.0,
            )
            .unwrap();
            let snd = sound_path.borrow().clone();
            match &existing_uid {
                // Editing keeps the UID — reminder dedup and any
                // external .ics consumer track appointments by it.
                Some(uid) => st.update(uid, &text, &desc_text, start, rems, snd.as_deref()),
                None => st.add(&text, &desc_text, start, rems, snd.as_deref()),
            }
            w.close();
        }
    });

    // The field list scrolls when it outgrows the window — the
    // action row stays pinned below.
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&body));
    scroll.set_max_content_height(480);
    scroll.set_propagate_natural_height(true);
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    outer.append(&scroll);
    outer.append(&buttons);
    win.set_child(Some(&outer));
    win.present();
    win
}
