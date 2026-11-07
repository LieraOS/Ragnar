// Notification-bell applet — reads the history written by the
// ragnar-raven notification server (org.freedesktop.Notifications).
// The taskbar deliberately does NOT own the bus name: raven is the
// single collector, the bell is a pure reader of its shared
// history.jsonl. The button is only visible while unread entries
// exist; opening the list marks them seen and hides it again.
use crate::bar::State;
use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// One history entry — mirrors ragnar-raven's JSONL record.
#[derive(Clone, serde::Deserialize)]
struct Note {
    app: String,
    summary: String,
    body: String,
    when: String,
}

type Notes = Rc<RefCell<Vec<Note>>>;

/// ragnar-raven's notification history (one JSON object per line).
fn history_file() -> std::path::PathBuf {
    glib::user_data_dir()
        .join("ragnar-raven")
        .join("history.jsonl")
}

/// Marker storing how many history lines were already seen —
/// the unread count is simply (total lines - seen lines).
fn seen_file() -> std::path::PathBuf {
    glib::user_data_dir().join("ragnar-raven").join(".seen")
}

/// Load the whole history (newest last in file order).
fn load() -> Vec<Note> {
    std::fs::read_to_string(history_file())
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn seen_count() -> usize {
    std::fs::read_to_string(seen_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn mark_seen(n: usize) {
    let dir = glib::user_data_dir().join("ragnar-raven");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(seen_file(), n.to_string());
}

/// The bell button applet.
pub fn build(state: &State, icon_px: i32, edge: &str) -> gtk4::Widget {
    let notes: Notes = Rc::new(RefCell::new(Vec::new()));
    let img = gtk4::Image::from_icon_name("xsi-notifications-symbolic");
    img.set_pixel_size(icon_px);
    let btn = gtk4::Button::new();
    btn.set_child(Some(&img));
    btn.add_css_class("notify-bell");
    btn.set_tooltip_text(Some(&crate::i18n::tr("Benachrichtigungen")));

    // Poll the history every 2 s: the bell is visible only while
    // entries exist that the user has not opened yet.
    {
        let (n, b2) = (notes.clone(), btn.clone());
        glib::timeout_add_seconds_local(2, move || {
            let all = load();
            let unread = all.len().saturating_sub(seen_count());
            *n.borrow_mut() = all;
            b2.set_visible(unread > 0);
            let tip = if unread == 0 {
                crate::i18n::tr("Benachrichtigungen")
            } else {
                crate::i18n::trf("{} ungelesene Benachrichtigungen", &[&unread.to_string()])
            };
            b2.set_tooltip_text(Some(&tip));
            if unread == 0 {
                b2.remove_css_class("has-notes");
            } else {
                b2.add_css_class("has-notes");
            }
            glib::ControlFlow::Continue
        });
    }

    let edge = edge.to_string();
    let _ = state;
    btn.connect_clicked(move |b| {
        let win = crate::menu::shell_at(b, &edge);
        let list = notes.borrow().clone();
        crate::menu::show(&win, &list_content(list));
        // Everything shown is now read — hide the bell again.
        mark_seen(notes.borrow().len());
    });
    btn.upcast()
}

/// The menu overlay content: the received notifications + a
/// clear-all row that truncates raven's history file.
fn list_content(list: Vec<Note>) -> gtk4::Widget {
    let boxv = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    boxv.set_size_request(320, -1);
    boxv.set_margin_top(8);
    boxv.set_margin_bottom(8);
    boxv.set_margin_start(8);
    boxv.set_margin_end(8);
    if list.is_empty() {
        boxv.append(&gtk4::Label::new(Some(&crate::i18n::tr(
            "Keine Benachrichtigungen",
        ))));
    } else {
        for note in list.iter().rev() {
            let row = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            let head = gtk4::Label::new(Some(&format!(
                "{} — {}  ({})",
                note.app, note.summary, note.when
            )));
            head.set_xalign(0.0);
            head.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row.append(&head);
            if !note.body.is_empty() {
                let b = gtk4::Label::new(Some(&note.body));
                b.set_xalign(0.0);
                b.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                b.add_css_class("dim-label");
                row.append(&b);
            }
            boxv.append(&row);
        }
        boxv.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        let clear = gtk4::Button::with_label(&crate::i18n::tr("Alle löschen"));
        clear.connect_clicked(move |_| {
            // History belongs to raven — truncating the file is the
            // shared, decoupled way to clear it.
            let _ = std::fs::write(history_file(), "");
            mark_seen(0);
            crate::menu::close_current();
        });
        boxv.append(&clear);
    }
    boxv.upcast()
}
