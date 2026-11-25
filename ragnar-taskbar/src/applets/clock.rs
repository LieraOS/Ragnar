// Copyright (C) Jörg Schröder
// Clock applet — HH:MM label refreshed once per second; the full date
// sits in the tooltip. Cinnamon-style interaction: LEFT click opens
// the calendar panel (date header, appointment area, month calendar,
// settings entry); RIGHT click opens the applet context menu
// (Einrichten). Display options (weekday/colour/weight/date format)
// live in clock_cfg and apply live on every tick.
use crate::applets::clock_cfg as cfg;
use crate::bar::State;
use gtk4::glib;
use gtk4::prelude::*;

pub fn build(_state: &State, vertical: bool) -> gtk4::Widget {
    let label = gtk4::Label::new(None);
    // On vertical bars the HH:MM string overflows the thin bar —
    // stack hour over minute instead. The weekday prefix honours
    // the configured mode (off/abbreviated/full).
    let tick_fmt = move || {
        let wd = cfg::weekday_prefix();
        if vertical {
            format!("{wd}%H\n%M")
        } else {
            format!("{wd}%H:%M")
        }
    };
    // Paint the text — plain set_text while colour and weight are
    // at their defaults; Pango markup takes over once either is
    // customised, so the untouched label renders exactly like the
    // theme wants it.
    let paint = move |l: &gtk4::Label, now: &glib::DateTime| {
        let txt = now.format(&tick_fmt()).unwrap();
        let (col, wt) = cfg::label_style();
        if col.is_none() && wt == cfg::Weight::Normal {
            l.set_text(&txt);
            return;
        }
        let mut span = String::from("<span");
        if let Some(c) = &col {
            span += &format!(" foreground=\"{c}\"");
        }
        if wt != cfg::Weight::Normal {
            span += &format!(" weight=\"{}\"", wt.pango());
        }
        l.set_markup(&format!("{span}>{}</span>", glib::markup_escape_text(&txt)));
    };
    {
        let now = glib::DateTime::now_local().unwrap();
        paint(&label, &now);
        label.set_tooltip_text(Some(&cfg::date_label(&now)));
    }
    let l = label.clone();
    glib::timeout_add_seconds_local(1, move || {
        // Re-read the format each tick so a weekday toggle applies
        // live without rebuilding the bar.
        let now = gtk4::glib::DateTime::now_local().unwrap();
        paint(&l, &now);
        glib::ControlFlow::Continue
    });

    // Left click → calendar panel.
    let left = gtk4::GestureClick::new();
    left.set_button(1);
    {
        let l = label.clone();
        left.connect_pressed(move |_, _, _, _| {
            let (lft, rgt, sw, h) = crate::menu::widget_span(&l);
            let win = crate::menu::shell(lft, rgt, sw, h);
            crate::menu::show(&win, &calendar_panel(&l));
        });
    }
    label.add_controller(left);

    // Right click → applet context menu — a single entry point into
    // the clock's own settings; applet management lives in the bar
    // settings, not per-applet.
    let right = gtk4::GestureClick::new();
    right.set_button(3);
    {
        let l = label.clone();
        right.connect_pressed(move |_, _, _, _| {
            let (lft, rgt, sw, h) = crate::menu::widget_span(&l);
            let win = crate::menu::shell(lft, rgt, sw, h);
            let items: Vec<(String, Box<dyn Fn() + 'static>)> = vec![(
                crate::i18n::tr("Einrichten …").into(),
                Box::new({
                    let l = l.clone();
                    move || crate::applets::clock_settings::open_settings(&l)
                }),
            )];
            crate::menu::show(&win, &crate::menu::item_list(items, &win));
        });
    }
    label.add_controller(right);

    label.upcast()
}

/// Left-click panel: current date and a live clock on top, the
/// month calendar underneath, plus a bottom entry point into the
/// date/time settings. Appointment display intentionally lives in
/// the Raven sidebar, not in this panel.
fn calendar_panel(anchor: &gtk4::Label) -> gtk4::Widget {
    let now = glib::DateTime::now_local().unwrap();
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    body.set_margin_top(8);
    body.set_margin_start(8);
    body.set_margin_end(8);

    // Date and clock above the calendar. The clock refreshes once
    // per second and stops automatically when the panel is dropped.
    let date = gtk4::Label::new(None);
    date.set_markup(&format!("<b>{}</b>", cfg::date_label(&now)));
    body.append(&date);
    let time = gtk4::Label::new(None);
    let fmt_time = |l: &gtk4::Label| {
        let t = glib::DateTime::now_local()
            .unwrap()
            .format("%H:%M:%S")
            .unwrap();
        l.set_markup(&format!("<span size=\"x-large\">{}</span>", t));
    };
    fmt_time(&time);
    {
        let weak = time.downgrade();
        glib::timeout_add_seconds_local(1, move || match weak.upgrade() {
            Some(l) => {
                fmt_time(&l);
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
    }
    body.append(&time);
    let cal = gtk4::Calendar::new();
    wire_calendar(&cal);
    body.append(&cal);
    root.append(&body);

    // Bottom: entry into the real date/time settings.
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    let settings = gtk4::Button::with_label(&crate::i18n::tr("Datums- und Zeiteinstellungen"));
    settings.set_has_frame(false);
    settings.set_halign(gtk4::Align::Start);
    {
        let a = anchor.clone();
        settings.connect_clicked(move |_| {
            crate::applets::clock_settings::open_settings(&a);
        });
    }
    root.append(&settings);

    root.upcast()
}

/// Path of the shared Raven appointment file. The taskbar only reads
/// it — ragnar-raven is the writer — so both components stay
/// independent and individually replaceable.
fn raven_ics() -> std::path::PathBuf {
    glib::user_data_dir()
        .join("ragnar-raven")
        .join("appointments.ics")
}

/// Day numbers of the currently displayed month that carry at least
/// one appointment — parsed straight from the Raven .ics file.
fn raven_marked_days(year: i32, month: i32) -> Vec<u32> {
    let mut days = Vec::new();
    if let Ok(text) = std::fs::read_to_string(raven_ics()) {
        for line in text.lines() {
            let line = line.trim_end();
            if !line.starts_with("DTSTART") {
                continue;
            }
            let Some(v) = line.rsplit(':').next() else {
                continue;
            };
            let v = v.trim_end_matches('Z');
            if v.len() < 8 {
                continue;
            }
            let (Ok(y), Ok(m), Ok(d)) = (
                v[0..4].parse::<i32>(),
                v[4..6].parse::<i32>(),
                v[6..8].parse::<u32>(),
            ) else {
                continue;
            };
            // gtk4::Calendar::month() is 0-based.
            if y == year && m == month + 1 {
                days.push(d);
            }
        }
    }
    days
}

/// Re-apply the appointment marks to the displayed month.
fn apply_marks(cal: &gtk4::Calendar) {
    cal.clear_marks();
    for d in raven_marked_days(cal.year(), cal.month()) {
        cal.mark_day(d);
    }
}

/// Gesture + marking wiring for the month calendar: appointment days
/// get marked, a double-click opens Raven on the selected day and the
/// right button opens the appointment/reminder menu.
fn wire_calendar(cal: &gtk4::Calendar) {
    apply_marks(cal);
    cal.connect_notify_local(Some("month"), |c, _| apply_marks(c));
    cal.connect_notify_local(Some("year"), |c, _| apply_marks(c));

    // Double-click a day → Raven opens at that day.
    let dbl = gtk4::GestureClick::new();
    dbl.set_button(1);
    dbl.connect_pressed({
        let cal = cal.clone();
        move |_, n, _, _| {
            if n == 2 {
                let day = cal.date().to_local().expect("local time conversion");
                open_raven(&day);
            }
        }
    });
    cal.add_controller(dbl);

    // Right click → appointment/reminder menu.
    let right = gtk4::GestureClick::new();
    right.set_button(3);
    right.connect_pressed({
        let cal = cal.clone();
        move |_, _, x, y| calendar_menu(&cal, x, y)
    });
    cal.add_controller(right);
}

/// Launch ragnar-raven for the given day. The `--day` argument is
/// delivered over D-Bus to a running instance (single-instance
/// GApplication); a cold start simply opens it directly.
///
/// The fullscreen calendar overlay must close first: while mapped
/// it swallows every pointer event above Layer::Top — Raven opens
/// below it and the first click on its window would be eaten by the
/// dismiss test instead of reaching the button.
fn open_raven(day: &glib::DateTime) {
    crate::menu::close_current();
    // glib's async spawn reaps the child automatically — a plain
    // Command::spawn left a zombie behind for every --day launch.
    let _ = glib::spawn_command_line_async(format!(
        "ragnar-raven --day={}",
        day.format("%Y%m%d").unwrap()
    ));
}

/// Raven's notification master switch — shared settings file that the
/// taskbar menu toggles and the Raven reminder part reads. Read
/// leniently: a strict GKeyFile parse rejects the whole file on one
/// malformed line, and other tools write this file too.
fn notifications_enabled() -> bool {
    let text = std::fs::read_to_string(
        glib::user_config_dir()
            .join("ragnar-raven")
            .join("settings.ini"),
    )
    .unwrap_or_default();
    let mut in_sec = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_sec = t == "[notifications]";
        } else if in_sec {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == "enabled" {
                    // Anything but an explicit false keeps it on —
                    // same fallback semantics as the KeyFile default.
                    return !matches!(v.trim(), "false" | "0");
                }
            }
        }
    }
    true
}

/// Write `enabled` into [notifications], preserving every other
/// line — a strict GKeyFile rewrite would wipe the file whenever
/// another tool left a malformed line behind.
fn set_notifications_enabled(on: bool) {
    let dir = glib::user_config_dir().join("ragnar-raven");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("settings.ini");
    let val = if on { "true" } else { "false" };
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let mut in_sec = false;
    let mut done = false;
    let mut insert_at = None;
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim().to_string();
        if t.starts_with('[') {
            if in_sec && !done {
                insert_at = Some(i);
            }
            in_sec = t == "[notifications]";
        } else if in_sec {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == "enabled" {
                    *line = format!("enabled = {val}");
                    done = true;
                }
            }
        }
    }
    if !done {
        match insert_at {
            Some(i) => lines.insert(i, format!("enabled = {val}")),
            None => {
                if !lines.iter().any(|l| l.trim() == "[notifications]") {
                    lines.push("[notifications]".to_string());
                }
                lines.push(format!("enabled = {val}"));
            }
        }
    }
    // Atomic write: temp + rename — a crash must not truncate the ini.
    let tmp = dir.join("settings.ini.tmp");
    if std::fs::write(&tmp, lines.join("\n") + "\n").is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Right-click menu on the calendar: open the Raven editor for the
/// selected day and toggle appointment notifications globally.
fn calendar_menu(cal: &gtk4::Calendar, x: f64, y: f64) {
    let pop = gtk4::Popover::new();
    pop.set_parent(cal);
    pop.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    let bx = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    bx.set_margin_top(6);
    bx.set_margin_bottom(6);
    bx.set_margin_start(6);
    bx.set_margin_end(6);

    let add = gtk4::Button::with_label(&crate::i18n::tr("Termin eintragen"));
    add.set_has_frame(false);
    add.connect_clicked({
        let (cal, pop) = (cal.clone(), pop.clone());
        move |_| {
            pop.popdown();
            let day = cal.date().to_local().expect("local time conversion");
            open_raven(&day);
        }
    });
    bx.append(&add);

    let toggle = gtk4::Button::with_label(&notify_label());
    toggle.set_has_frame(false);
    toggle.connect_clicked(|b| {
        let on = !notifications_enabled();
        set_notifications_enabled(on);
        b.set_label(&notify_label());
    });
    bx.append(&toggle);

    pop.set_child(Some(&bx));
    pop.popup();
}

/// Menu caption reflecting the current notification master switch.
fn notify_label() -> String {
    crate::i18n::tr(if notifications_enabled() {
        "Benachrichtigungen ausschalten"
    } else {
        "Benachrichtigungen einschalten"
    })
}
