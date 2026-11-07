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
pub fn open_settings(_parent: &gtk4::Label) {
    let now = glib_now();

    let dialog = gtk4::Window::builder()
        .title("Datum & Uhrzeit")
        .modal(false)
        .default_width(360)
        .build();
    let grid = gtk4::Grid::new();
    grid.set_margin_top(12);
    grid.set_margin_bottom(12);
    grid.set_margin_start(12);
    grid.set_margin_end(12);
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

    // Display option: weekday prefix in the bar label — a clock.ini
    // setting, applied live via the applet flag.
    let wd = gtk4::CheckButton::with_label(&crate::i18n::tr("Wochentag anzeigen"));
    wd.set_active(crate::applets::clock::weekday_saved());
    wd.connect_toggled(|b| crate::applets::clock::set_show_weekday(b.is_active()));
    grid.attach(&wd, 0, 4, 2, 1);

    // Async load: current props + timezone list populate the fields
    // as soon as timedatectl answers — the dialog is live meanwhile.
    {
        let (zone, names, sw, lbl) = (
            zone.clone(),
            zone_names.clone(),
            ntp_sw.clone(),
            ntp_lbl.clone(),
        );
        gtk4::glib::spawn_future_local(async move {
            let props = tctl(&["show".to_string()]).await.unwrap_or_default();
            let map: std::collections::HashMap<String, String> = props
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.to_string(), v.trim().to_string()))
                .collect();
            let tz = map.get("Timezone").cloned().unwrap_or_default();
            // timedatectl reports no NTPSynchronized key at all when
            // no NTP daemon exists — the switch stays dead then.
            let ntp_supported = map.contains_key("NTPSynchronized");
            sw.set_active(
                map.get("NTPSynchronized")
                    .map(|v| v == "yes")
                    .unwrap_or(false),
            );
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
        });
    }

    let apply = gtk4::Button::with_label(&crate::i18n::tr("Übernehmen"));
    apply.set_halign(gtk4::Align::End);
    {
        let (d, t, z, ntp, zones, dlg) = (
            date.clone(),
            time.clone(),
            zone.clone(),
            ntp_sw.clone(),
            zone_names.clone(),
            dialog.clone(),
        );
        apply.connect_clicked(move |b| {
            // Async: set-time/set-timezone may raise a polkit prompt
            // that blocks until the user authenticates — the bar must
            // keep running meanwhile.
            let (b, dlg) = (b.clone(), dlg.clone());
            let (d, t, z, ntp, zones) =
                (d.clone(), t.clone(), z.clone(), ntp.clone(), zones.clone());
            gtk4::glib::spawn_future_local(async move {
                let mut errs = Vec::new();
                let mut calls = vec![vec![
                    "set-ntp".into(),
                    if ntp.is_active() {
                        "true".into()
                    } else {
                        "false".into()
                    },
                ]];
                if !ntp.is_active() {
                    calls.push(vec![
                        "set-time".into(),
                        format!("{} {}", d.text(), t.text()),
                    ]);
                }
                if let Some(name) = zones.borrow().get(z.selected() as usize).cloned() {
                    calls.push(vec!["set-timezone".into(), name]);
                }
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
    grid.attach(&apply, 0, 5, 2, 1);
    dialog.set_child(Some(&grid));
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
