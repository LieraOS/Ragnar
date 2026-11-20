// SPDX-License-Identifier: GPL-3.0-or-later
//! Process details dialog + right-click menu — attribute rows,
//! nice-priority control and terminate/kill actions. Signals go
//! through libc directly; renice runs via procps so glibc's unstable
//! setpriority ABI stays out of the binary.

use gettextrs::gettext as tr;
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::util::kb_human;
use crate::procfs::Process;

/// Right-click menu for a process row: end, kill, details.
pub fn ctx_menu(row: &gtk4::ListBoxRow, p: &Process) {
    let pop = gtk4::Popover::new();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    for (label, sig) in [
        (tr("End Task"), libc::SIGTERM),
        (tr("Kill Process"), libc::SIGKILL),
    ] {
        let btn = gtk4::Button::with_label(&label);
        btn.add_css_class("flat");
        let pid = p.pid;
        let pop = pop.clone();
        btn.connect_clicked(move |_| {
            unsafe { libc::kill(pid, sig) };
            pop.popdown();
        });
        b.append(&btn);
    }
    let det = gtk4::Button::with_label(&tr("Details"));
    det.add_css_class("flat");
    let p2 = p.clone();
    let pop2 = pop.clone();
    let row2 = row.clone();
    det.connect_clicked(move |_| {
        if let Some(r) = row2.root() {
            details(&r, p2.clone());
        }
        pop2.popdown();
    });
    b.append(&det);
    pop.set_child(Some(&b));
    pop.set_parent(row);
    pop.popup();
}

/// Details dialog — attribute rows, nice control, end/kill buttons.
pub fn details(root: &gtk4::Root, p: Process) {
    let win = root.downcast_ref::<gtk4::Window>().cloned();
    let mut b = gtk4::Window::builder()
        .title(&format!("{} — {}", p.name, p.pid))
        .modal(true)
        .default_width(460);
    if let Some(w) = &win {
        b = b.transient_for(w);
    }
    let dlg = b.build();
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    outer.append(&info_group(&p));

    let prio = gtk4::SpinButton::with_range(-20.0, 19.0, 1.0);
    prio.set_value(p.nice as f64);
    let prio_btn = gtk4::Button::with_label(&tr("Apply"));
    let prio_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    prio_box.append(
        &gtk4::Label::builder()
            .label(&tr("Priority (nice):"))
            .hexpand(true)
            .halign(gtk4::Align::Start)
            .build(),
    );
    prio_box.append(&prio);
    prio_box.append(&prio_btn);
    outer.append(&prio_box);
    let msg = gtk4::Label::new(None);
    msg.add_css_class("error");
    outer.append(&msg);

    prio_btn.connect_clicked({
        let msg = msg.clone();
        let pid = p.pid;
        let prio = prio.clone();
        move |_| match renice(pid, prio.value() as i32) {
            Ok(()) => {
                msg.remove_css_class("error");
                msg.set_text(&tr("Priority updated"))
            }
            Err(e) => {
                msg.add_css_class("error");
                msg.set_text(&e)
            }
        }
    });

    let btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    for (label, sig, cls) in [
        (tr("End Task"), libc::SIGTERM, ""),
        (tr("Kill Process"), libc::SIGKILL, "destructive-action"),
    ] {
        let btn = gtk4::Button::with_label(&label);
        if !cls.is_empty() {
            btn.add_css_class(cls)
        }
        let msg = msg.clone();
        let pid = p.pid;
        let dlg = dlg.clone();
        btn.connect_clicked(move |_| {
            if unsafe { libc::kill(pid, sig) } == 0 {
                dlg.close()
            } else {
                msg.add_css_class("error");
                msg.set_text(&tr("Operation not permitted"))
            }
        });
        btns.append(&btn);
    }
    outer.append(&btns);
    dlg.set_child(Some(&outer));
    dlg.present();
}

/// Read-only attribute rows of the details dialog.
fn info_group(p: &Process) -> libadwaita::PreferencesGroup {
    let g = libadwaita::PreferencesGroup::new();
    for (k, v) in [
        (tr("Name"), p.name.clone()),
        (tr("PID"), p.pid.to_string()),
        (tr("Parent PID"), p.ppid.to_string()),
        (tr("User"), p.user.clone()),
        (tr("State"), crate::util::state_name(p.state).to_string()),
        (tr("Threads"), p.threads.to_string()),
        (tr("Memory"), kb_human(p.rss_kb)),
        (tr("Started"), started_at(p.pid)),
        (tr("Command"), p.cmdline.clone()),
    ] {
        g.add(
            &libadwaita::ActionRow::builder()
                .title(&k)
                .subtitle(&v)
                .subtitle_selectable(true)
                .build(),
        );
    }
    g
}

/// Unix seconds when the pid started → local time string.
fn started_at(pid: i32) -> String {
    let secs = crate::procfs::proc_start_secs(pid, crate::procfs::boot_seconds());
    glib::DateTime::from_unix_local(secs as i64)
        .and_then(|d| d.format("%x %X").map(|s| s.to_string()))
        .unwrap_or_default()
}

/// renice wrapper — returns the tool's stderr on failure.
fn renice(pid: i32, nice: i32) -> Result<(), String> {
    let o = std::process::Command::new("renice")
        .args(["-n", &nice.to_string(), "-p", &pid.to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}
