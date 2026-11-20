// Date & time settings dialog — the real backend is timedatectl, the
// standard non-desktop mechanism (polkit may prompt for auth).
// Reads current state via `timedatectl show`, offers date/time entry,
// timezone dropdown and an NTP switch.
// Every timedatectl call runs through gio::Subprocess — asynchronous:
// a polkit prompt can block for minutes waiting on the password, and
// that wait must never freeze the bar's main loop.
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Run timedatectl off the main loop; returns stdout on success,
/// stderr text on failure so the caller can surface real errors.
async fn tctl(args: &[String]) -> Result<String, String> {
    let argv: Vec<std::ffi::OsString> = std::iter::once(std::ffi::OsString::from("timedatectl"))
        .chain(args.iter().map(std::ffi::OsString::from))
        .collect();
    let refs: Vec<&std::ffi::OsStr> = argv.iter().map(|a| a.as_os_str()).collect();
    let p = gio::Subprocess::newv(
        &refs,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    )
    .map_err(|e| e.to_string())?;
    let (stdout, stderr) = p
        .communicate_utf8_future(None)
        .await
        .map_err(|e| e.to_string())?;
    if p.exit_status() == 0 {
        Ok(stdout.map(|g| g.to_string()).unwrap_or_default())
    } else {
        Err(stderr
            .map(|g| g.to_string().trim().to_string())
            .unwrap_or_default())
    }
}

/// Open the settings popover under the clock label. The dialog is
/// shown immediately; timedatectl state fills in asynchronously.
/// A live clock on top always shows the *current* system time —
/// the entry fields below are for *setting*, they cannot tick or
/// they would fight the user's input.
pub fn open_settings(_parent: &gtk4::Label) {
    let now = glib_now();

    let dialog = gtk4::Window::builder()
        .title(&crate::i18n::tr("Datum & Uhrzeit"))
        .modal(false)
        .default_width(360)
        .build();
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    // Live system time — ticks once per second until the dialog
    // is destroyed (weak ref stops the timer).
    let live = gtk4::Label::new(None);
    live.set_halign(gtk4::Align::Center);
    let paint_live = |l: &gtk4::Label| {
        let now = gtk4::glib::DateTime::now_local().unwrap();
        l.set_markup(&format!(
            "<b>{} — {}</b>",
            crate::applets::clock_cfg::date_label(&now),
            now.format("%H:%M:%S").unwrap()
        ));
    };
    paint_live(&live);
    {
        let weak = live.downgrade();
        gtk4::glib::timeout_add_seconds_local(1, move || match weak.upgrade() {
            Some(l) => {
                paint_live(&l);
                gtk4::glib::ControlFlow::Continue
            }
            None => gtk4::glib::ControlFlow::Break,
        });
    }
    vbox.append(&live);
    vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let grid = gtk4::Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(12);

    let date = gtk4::Entry::new();
    date.set_text(&now.0);
    grid.attach(
        &gtk4::Label::new(Some(&crate::i18n::tr("Datum (JJJJ-MM-TT)"))),
        0,
        0,
        1,
        1,
    );
    grid.attach(&date, 1, 0, 1, 1);

    let time = gtk4::Entry::new();
    time.set_text(&now.1);
    grid.attach(
        &gtk4::Label::new(Some(&crate::i18n::tr("Uhrzeit (SS:MM:SS)"))),
        0,
        1,
        1,
        1,
    );
    grid.attach(&time, 1, 1, 1, 1);

    // Timezone dropdown — its model arrives with the async load.
    let zone = gtk4::DropDown::new(None::<gtk4::StringList>, None::<&gtk4::Expression>);
    let zone_names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    grid.attach(
        &gtk4::Label::new(Some(&crate::i18n::tr("Zeitzone"))),
        0,
        2,
        1,
        1,
    );
    grid.attach(&zone, 1, 2, 1, 1);

    let ntp_sw = gtk4::Switch::new();
    ntp_sw.set_halign(gtk4::Align::Start);
    // Disabled until the async probe proves an NTP service exists.
    ntp_sw.set_sensitive(false);
    let ntp_lbl = gtk4::Label::new(Some(&crate::i18n::tr("Netzwerkzeit (NTP)")));
    grid.attach(&ntp_lbl, 0, 3, 1, 1);
    grid.attach(&ntp_sw, 1, 3, 1, 1);

    // Initial system state as probed below — Apply computes a diff
    // against it and only calls timedatectl for what the user
    // actually changed, so an unchanged dialog never triggers a
    // polkit prompt.
    let init: Rc<RefCell<Option<(bool, String)>>> = Rc::new(RefCell::new(None));

    let apply = gtk4::Button::with_label(&crate::i18n::tr("Übernehmen"));
    apply.set_halign(gtk4::Align::End);
    // Stays locked until the probe has delivered the initial state —
    // applying earlier could disable NTP just because the switch
    // still shows its inactive default.
    apply.set_sensitive(false);

    // Async load: current props + timezone list populate the fields
    // as soon as timedatectl answers — the dialog is live meanwhile.
    {
        let (zone, names, sw, lbl, init, ap) = (
            zone.clone(),
            zone_names.clone(),
            ntp_sw.clone(),
            ntp_lbl.clone(),
            init.clone(),
            apply.clone(),
        );
        gtk4::glib::spawn_future_local(async move {
            let props = tctl(&["show".to_string()]).await.unwrap_or_default();
            let map: std::collections::HashMap<String, String> = props
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.to_string(), v.trim().to_string()))
                .collect();
            let tz = map.get("Timezone").cloned().unwrap_or_default();
            // `NTP` is the enable flag; `NTPSynchronized` only says
            // whether a sync already succeeded — a fresh boot with a
            // running timesyncd must still show the switch ON.
            let ntp_supported = map.contains_key("NTP") || map.contains_key("NTPSynchronized");
            let ntp_on = map.get("NTP").map(|v| v == "yes").unwrap_or_else(|| {
                map.get("NTPSynchronized")
                    .map(|v| v == "yes")
                    .unwrap_or(false)
            });
            sw.set_active(ntp_on);
            sw.set_sensitive(ntp_supported);
            if !ntp_supported {
                lbl.set_tooltip_text(Some(&crate::i18n::tr(
                    "Kein NTP-Dienst auf dem System installiert",
                )));
            }
            let list = tctl(&["list-timezones".to_string()])
                .await
                .unwrap_or_default();
            let zs: Vec<String> = list.lines().map(|s| s.to_string()).collect();
            let pos = zs.iter().position(|z| *z == tz).map(|i| i as u32);
            let model = gtk4::StringList::new(&zs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            zone.set_model(Some(&model));
            if let Some(i) = pos {
                zone.set_selected(i);
            }
            *names.borrow_mut() = zs;
            *init.borrow_mut() = Some((ntp_on, tz));
            ap.set_sensitive(true);
        });
    }

    // With NTP on, manual date/time input is ignored by timedated —
    // lock the fields while the switch is active so nothing is
    // typed that would be silently discarded.
    {
        let (d, t) = (date.clone(), time.clone());
        ntp_sw.connect_active_notify(move |sw| {
            let editable = !sw.is_active();
            d.set_sensitive(editable);
            t.set_sensitive(editable);
            let tip = if editable {
                None
            } else {
                Some(crate::i18n::tr("Nur änderbar, wenn Netzwerkzeit (NTP) aus ist"))
            };
            d.set_tooltip_text(tip.as_deref());
            t.set_tooltip_text(tip.as_deref());
        });
    }

    {
        let (d, t, z, ntp, zones, dlg, init) = (
            date.clone(),
            time.clone(),
            zone.clone(),
            ntp_sw.clone(),
            zone_names.clone(),
            dialog.clone(),
            init.clone(),
        );
        // Field contents as shown when the dialog opened — the
        // set-time diff compares against these, never against the
        // live widget text (that would always match itself).
        let (init_date, init_time) = (now.0.clone(), now.1.clone());
        apply.connect_clicked(move |b| {
            // Async: set-time/set-timezone may raise a polkit prompt
            // that blocks until the user authenticates — the bar must
            // keep running meanwhile.
            let (b, dlg) = (b.clone(), dlg.clone());
            let (d, t, z, ntp, zones, init, init_date, init_time) = (
                d.clone(),
                t.clone(),
                z.clone(),
                ntp.clone(),
                zones.clone(),
                init.clone(),
                init_date.clone(),
                init_time.clone(),
            );
            gtk4::glib::spawn_future_local(async move {
                // Only system settings that really differ from the
                // probed state get a timedatectl call — nothing
                // changed means no call and no polkit prompt.
                let mut calls: Vec<Vec<String>> = Vec::new();
                let ntp_on = ntp.is_active();
                if let Some((init_ntp, init_tz)) = init.borrow().as_ref() {
                    if ntp_on != *init_ntp {
                        calls.push(vec![
                            "set-ntp".into(),
                            if ntp_on { "true".into() } else { "false".into() },
                        ]);
                    }
                    let date = d.text().to_string();
                    let time = t.text().to_string();
                    if !ntp_on && (date != init_date || time != init_time) {
                        calls.push(vec!["set-time".into(), format!("{date} {time}")]);
                    }
                    if let Some(name) = zones.borrow().get(z.selected() as usize).cloned() {
                        if name != *init_tz {
                            calls.push(vec!["set-timezone".into(), name]);
                        }
                    }
                }
                if calls.is_empty() {
                    dlg.close();
                    return;
                }
                let mut errs = Vec::new();
                for args in calls {
                    if let Err(e) = tctl(&args).await {
                        errs.push(e);
                    }
                }
                if errs.is_empty() {
                    dlg.close();
                } else {
                    b.set_label(&format!(
                        "Fehler: {}",
                        errs.first().cloned().unwrap_or_default()
                    ));
                }
            });
        });
    }
    vbox.append(&grid);

    // Bar-label display options (weekday/weight/colour/date
    // format) — own module: they never touch the system clock
    // and therefore never need polkit or the Apply button.
    vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    vbox.append(&crate::applets::clock_prefs::section());
    vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    vbox.append(&apply);
    dialog.set_child(Some(&vbox));
    dialog.present();
}

/// Current local date/time strings for the entry fields.
fn glib_now() -> (String, String) {
    let now = gtk4::glib::DateTime::now_local().unwrap();
    (
        now.format("%Y-%m-%d").unwrap().to_string(),
        now.format("%H:%M:%S").unwrap().to_string(),
    )
}
