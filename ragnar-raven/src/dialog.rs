//! Appointment editor dialog: title, start time and an arbitrary set
//! of reminder offsets. Opened for the day the user picked — either
//! by double-clicking a calendar day or via the right-click menu.

use gtk4::glib;
use gtk4::prelude::*;
use std::collections::BTreeSet;

use crate::i18n::tr;
use crate::store::{Reminder, Store};

/// Human-readable label for a reminder offset given in minutes.
fn offset_label(minutes: i64) -> String {
    match minutes {
        0 => tr("Zum Terminzeitpunkt"),
        m if m % 1440 == 0 => format!("{} {}", m / 1440, tr("Tage vorher")),
        m if m % 60 == 0 => format!("{} {}", m / 60, tr("Stunden vorher")),
        m => format!("{} {}", m, tr("Minuten vorher")),
    }
}

/// Show the new-appointment editor anchored to `parent`, pre-filled
/// for the calendar day `day`. On save the appointment goes straight
/// into the shared store (which persists to the .ics file). Returns
/// the editor window so the caller can react to it being closed.
pub fn open_editor(
    parent: &gtk4::Widget,
    day: &glib::DateTime,
    store: &Store,
) -> gtk4::Window {
    let win = gtk4::Window::builder()
        .title(tr("Neuer Termin"))
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
    headline.set_markup(&format!(
        "<b>{}</b>",
        day.format("%A, %d. %B %Y").unwrap()
    ));
    body.append(&headline);

    // Title.
    let title = gtk4::Entry::builder()
        .placeholder_text(tr("Titel des Termins"))
        .build();
    body.append(&title);

    // Start time as two spin buttons (hour / minute), pre-filled
    // with the current time and rendered zero-padded (08:00).
    let now_local = glib::DateTime::now_local().unwrap();
    let time_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    time_row.append(&gtk4::Label::new(Some(&tr("Uhrzeit"))));
    let hour = gtk4::SpinButton::with_range(0.0, 23.0, 1.0);
    hour.set_value(now_local.hour() as f64);
    hour.connect_output(|sb| {
        sb.set_text(&format!("{:02}", sb.value() as i32));
        glib::Propagation::Stop
    });
    let colon = gtk4::Label::new(Some(":"));
    let minute = gtk4::SpinButton::with_range(0.0, 59.0, 1.0);
    minute.set_numeric(true);
    minute.set_value(now_local.minute() as f64);
    minute.connect_output(|sb| {
        sb.set_text(&format!("{:02}", sb.value() as i32));
        glib::Propagation::Stop
    });
    time_row.append(&hour);
    time_row.append(&colon);
    time_row.append(&minute);
    body.append(&time_row);

    // Reminder offsets: preset check boxes plus a free-form entry so
    // any offset (e.g. 90 minutes) is possible too. Multiple entries
    // are allowed and fire independently in chronological order.
    let rem_head = gtk4::Label::new(Some(&tr("Erinnern")));
    rem_head.set_halign(gtk4::Align::Start);
    body.append(&rem_head);
    let mut checks = Vec::new();
    for &m in Reminder::PRESETS {
        let cb = gtk4::CheckButton::with_label(&offset_label(m));
        body.append(&cb);
        checks.push((cb, m));
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

    // Actions.
    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    buttons.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&tr("Abbrechen"));
    let save = gtk4::Button::with_label(&tr("Speichern"));
    save.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&save);
    body.append(&buttons);

    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    save.connect_clicked({
        let (w, st, d) = (win.clone(), store.clone(), day.clone());
        move |_| {
            let text = title.text().trim().to_string();
            if text.is_empty() {
                return;
            }
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
            st.add(&text, start, rems);
            w.close();
        }
    });

    win.set_child(Some(&body));
    win.present();
    win
}
