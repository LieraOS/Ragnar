// Copyright (C) Jörg Schröder
//! Diagnostics page: the user selects which check suites to run and
//! starts the battery explicitly — nothing runs on page open. The
//! battery itself lives in [`backend::diag`] and runs in a worker;
//! results render grouped by area with severity icons.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Image, Spinner};
use libadwaita as adw;

use adw::prelude::*;
use adw::{ActionRow, PreferencesGroup, PreferencesPage, SwitchRow};

use crate::backend::diag::{self, Check, Status};

/// Poll interval for the diagnostics worker result.
const POLL_MS: u64 = 80;

/// Build the diagnostics page: a one-click quick battery for normal
/// users first, then the full switchable battery (standard + expert
/// tiers), a run button and a report export. Nothing executes until
/// a button is activated.
pub fn build() -> gtk4::Widget {
    let page = PreferencesPage::new();
    let groups: Rc<RefCell<BTreeMap<&'static str, PreferencesGroup>>> =
        Rc::new(RefCell::new(BTreeMap::new()));
    let running = Rc::new(Cell::new(false));
    // Last finished battery — feeds the markdown export.
    let last_checks: Rc<RefCell<Vec<Check>>> = Rc::new(RefCell::new(Vec::new()));

    // Quick battery — the everyday "is my network okay" set, one
    // click, no switch fiddling.
    let quick_group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Quick diagnosis"))
        .description(gettextrs::gettext(
            "Runs the essential checks: links, addresses, gateway, DNS, \
             services, Wi-Fi and internet speed.",
        ))
        .build();
    let spinner = Spinner::builder().visible(false).build();
    let quick_row = ActionRow::builder()
        .title(gettextrs::gettext("Run quick diagnosis"))
        .activatable(true)
        .build();
    quick_row.add_prefix(&Image::from_icon_name("system-run-symbolic"));
    quick_row.add_suffix(&spinner);
    quick_group.add(&quick_row);
    page.add(&quick_group);

    // Standard + expert switches share one collection; the groups
    // differ only in their tier.
    let mut switches = Vec::new();
    let std_group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Standard checks"))
        .build();
    let exp_group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Expert checks"))
        .description(gettextrs::gettext(
            "Deep-dive diagnostics for experienced users: portals, IPv6, \
             path/MTU, neighbors, listeners, VPN, DNS and radio details.",
        ))
        .build();
    for suite in diag::SUITES {
        let row = SwitchRow::builder()
            .title(gettextrs::gettext(suite.title))
            .subtitle(gettextrs::gettext(suite.description))
            .active(true)
            .build();
        match suite.tier {
            diag::Tier::Quick => std_group.add(&row),
            diag::Tier::Expert => exp_group.add(&row),
        }
        switches.push((suite.id, row));
    }
    page.add(&std_group);
    page.add(&exp_group);
    let switches = Rc::new(switches);

    // Run + export actions.
    let run_group = PreferencesGroup::new();
    let run_row = ActionRow::builder()
        .title(gettextrs::gettext("Run selected diagnostics"))
        .subtitle(gettextrs::gettext("Execute all enabled checks"))
        .activatable(true)
        .build();
    run_row.add_prefix(&Image::from_icon_name("system-run-symbolic"));
    let save_row = ActionRow::builder()
        .title(gettextrs::gettext("Save report…"))
        .subtitle(gettextrs::gettext("Export the last diagnosis as markdown"))
        .activatable(true)
        .sensitive(false)
        .build();
    save_row.add_prefix(&Image::from_icon_name("document-save-symbolic"));
    run_group.add(&run_row);
    run_group.add(&save_row);
    page.add(&run_group);

    // Wire the two run buttons to the same worker.
    for (row, preset) in [(quick_row, true), (run_row, false)] {
        let groups = Rc::clone(&groups);
        let switches = Rc::clone(&switches);
        let running = Rc::clone(&running);
        let spinner = spinner.clone();
        let page = page.clone();
        let last_checks = Rc::clone(&last_checks);
        let save_row = save_row.clone();
        row.connect_activated(move |_| {
            if running.get() {
                return;
            }
            let enabled: BTreeSet<&'static str> = if preset {
                diag::QUICK_SUITES.iter().copied().collect()
            } else {
                switches
                    .iter()
                    .filter(|(_, row)| row.is_active())
                    .map(|(id, _)| *id)
                    .collect()
            };
            if enabled.is_empty() {
                return;
            }
            run_checks(
                &page,
                &groups,
                &spinner,
                &running,
                &last_checks,
                &save_row,
                enabled,
            );
        });
    }

    // Report export — markdown via the portal-free file dialog.
    {
        let last_checks = Rc::clone(&last_checks);
        save_row.connect_activated(move |row| {
            let Some(win) = row.root().and_downcast::<gtk4::Window>() else {
                return;
            };
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let dialog = gtk4::FileDialog::builder()
                .title(gettextrs::gettext("Save diagnosis report"))
                .initial_name(format!("heimdall-diagnosis-{stamp}.md"))
                .build();
            let checks = last_checks.borrow().clone();
            gtk4::glib::MainContext::default().spawn_local(async move {
                if let Ok(file) = dialog.save_future(Some(&win)).await {
                    if let Some(path) = file.path() {
                        let md = diag::report_md(&checks);
                        if let Err(err) = std::fs::write(&path, md) {
                            tracing::warn!("report save failed: {err:#}");
                        }
                    }
                }
            });
        });
    }
    page.upcast()
}

/// Start the selected battery in a worker; re-render on completion
/// and arm the export button once results exist.
fn run_checks(
    page: &PreferencesPage,
    groups: &Rc<RefCell<BTreeMap<&'static str, PreferencesGroup>>>,
    spinner: &Spinner,
    running: &Rc<Cell<bool>>,
    last_checks: &Rc<RefCell<Vec<Check>>>,
    save_row: &ActionRow,
    enabled: BTreeSet<&'static str>,
) {
    running.set(true);
    clear_groups(page, groups);
    spinner.set_visible(true);
    spinner.start();
    save_row.set_sensitive(false);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(diag::run(&enabled));
    });

    let page = page.clone();
    let groups = Rc::clone(groups);
    let spinner = spinner.clone();
    let running = Rc::clone(running);
    let last_checks = Rc::clone(last_checks);
    let save_row = save_row.clone();
    gtk4::glib::timeout_add_local(Duration::from_millis(POLL_MS), move || {
        match rx.try_recv() {
            Ok(checks) => {
                spinner.stop();
                spinner.set_visible(false);
                running.set(false);
                *last_checks.borrow_mut() = checks.clone();
                save_row.set_sensitive(true);
                render(&page, &groups, &checks);
                gtk4::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                spinner.stop();
                running.set(false);
                gtk4::glib::ControlFlow::Break
            }
        }
    });
}

/// Render the finished battery: a summary header first (so the
/// verdict is visible without scrolling), then one
/// PreferencesGroup per area, one row per check with a severity icon.
fn render(
    page: &PreferencesPage,
    groups: &Rc<RefCell<BTreeMap<&'static str, PreferencesGroup>>>,
    checks: &[Check],
) {
    clear_groups(page, groups);
    let mut map = groups.borrow_mut();

    // Verdict header: counts per severity, worst result first.
    let (mut pass, mut warn, mut fail, mut skip) = (0u32, 0u32, 0u32, 0u32);
    for c in checks {
        match c.status {
            Status::Pass => pass += 1,
            Status::Warn => warn += 1,
            Status::Fail => fail += 1,
            Status::Skip => skip += 1,
        }
    }
    let (icon, title) = if fail > 0 {
        (
            "dialog-error-symbolic",
            format!("{fail} {}", gettextrs::gettext("problem(s) found")),
        )
    } else if warn > 0 {
        (
            "dialog-warning-symbolic",
            format!("{warn} {}", gettextrs::gettext("warning(s)")),
        )
    } else {
        (
            "emblem-ok-symbolic",
            gettextrs::gettext("all checks passed"),
        )
    };
    let summary = PreferencesGroup::new();
    let row = ActionRow::builder()
        .title(title)
        .subtitle(format!(
            "{pass} ok · {warn} {} · {fail} {} · {skip} {}",
            gettextrs::gettext("warn"),
            gettextrs::gettext("fail"),
            gettextrs::gettext("skipped")
        ))
        .build();
    let img = Image::from_icon_name(icon);
    if fail > 0 {
        img.add_css_class("error");
    } else if warn > 0 {
        img.add_css_class("warning");
    } else {
        img.add_css_class("success");
    }
    row.add_prefix(&img);
    summary.add(&row);
    page.add(&summary);
    map.insert("0summary", summary);

    for check in checks {
        let group = map.entry(check.group).or_insert_with(|| {
            let g = PreferencesGroup::builder()
                .title(gettextrs::gettext(check.group))
                .build();
            page.add(&g);
            g
        });
        group.add(&check_row(check));
    }
}

/// One result row: severity icon, check name, detail line — names
/// and details run through gettext so the German catalog can cover
/// the fixed parts while dynamic values pass through unchanged.
fn check_row(check: &Check) -> ActionRow {
    let row = ActionRow::builder()
        .title(gettextrs::gettext(&check.name))
        .subtitle(gettextrs::gettext(&check.detail))
        .build();
    let (icon, style) = match check.status {
        Status::Pass => ("emblem-ok-symbolic", "success"),
        Status::Warn => ("dialog-warning-symbolic", "warning"),
        Status::Fail => ("dialog-error-symbolic", "error"),
        Status::Skip => ("dialog-question-symbolic", "dim-label"),
    };
    let image = Image::from_icon_name(icon);
    image.add_css_class(style);
    row.add_prefix(&image);
    row
}

/// Drop all rendered result groups (selection and run groups stay —
/// they live outside the map).
fn clear_groups(
    page: &PreferencesPage,
    groups: &Rc<RefCell<BTreeMap<&'static str, PreferencesGroup>>>,
) {
    for (_, group) in std::mem::take(&mut *groups.borrow_mut()) {
        page.remove(&group);
    }
}
