// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings page: Flathub toggle + sources editor (deb822, pending
//! changes → one pkexec apply, one password).

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

struct Src {
    file: String,
    uris: String,
    suites: String,
    comps: String,
    deb822: bool,
}

type Ops = Rc<RefCell<Vec<String>>>;

pub struct SettingsPage {
    pub widget: gtk4::Widget,
}

impl SettingsPage {
    pub fn new() -> Self {
        let grp = adw::PreferencesGroup::new();
        grp.set_title(&gettext("Software-Quellen"));
        let fp = adw::ActionRow::new();
        fp.set_title("Flathub");
        fp.set_subtitle(&gettext("Flathub-Apps im Katalog anzeigen"));
        let sw = gtk4::Switch::new();
        sw.set_active(
            std::process::Command::new("flatpak")
                .args(["remotes", "--user", "--columns=name,options"])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .any(|l| l.starts_with("flathub") && !l.contains("disabled"))
                })
                .unwrap_or(false),
        );
        sw.set_valign(gtk4::Align::Center);
        sw.connect_active_notify(|s| {
            let _ = std::process::Command::new("flatpak")
                .args([
                    "remote-modify",
                    "--user",
                    if s.is_active() {
                        "--enable"
                    } else {
                        "--disable"
                    },
                    "flathub",
                ])
                .output();
        });
        fp.add_suffix(&sw);
        grp.add(&fp);
        let sr = adw::ActionRow::new();
        sr.set_title(&gettext("Quellen"));
        sr.set_subtitle(&gettext("APT-Quellen anzeigen und bearbeiten"));
        sr.set_activatable(true);
        let arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        arrow.set_valign(gtk4::Align::Center);
        sr.add_suffix(&arrow);
        grp.add(&sr);
        let col = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        col.set_margin_start(12);
        col.set_margin_end(12);
        col.set_margin_top(12);
        col.set_margin_bottom(12);
        col.append(&grp);
        sr.connect_activated(move |row| {
            if let Some(root) = row.root() {
                open_sources(&root);
            }
        });
        Self {
            widget: col.upcast(),
        }
    }
}

fn read_sources() -> Vec<Src> {
    let mut out = Vec::new();
    let mut files = vec!["/etc/apt/sources.list".to_string()];
    if let Ok(rd) = std::fs::read_dir("/etc/apt/sources.list.d") {
        for e in rd.flatten() {
            let p = e.path().to_string_lossy().to_string();
            if p.ends_with(".list") || p.ends_with(".sources") {
                files.push(p);
            }
        }
    }
    for f in files {
        let short = std::path::Path::new(&f)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(f.clone());
        let Ok(txt) = std::fs::read_to_string(&f) else {
            continue;
        };
        if f.ends_with(".sources") {
            let (mut uris, mut sui, mut comps) = (String::new(), String::new(), String::new());
            for l in txt.lines().chain(std::iter::once("")) {
                if l.trim().is_empty() {
                    if !uris.is_empty() {
                        out.push(Src {
                            file: short.clone(),
                            uris,
                            suites: sui,
                            comps,
                            deb822: true,
                        });
                    }
                    uris = String::new();
                    sui = String::new();
                    comps = String::new();
                    continue;
                }
                if let Some(v) = l.strip_prefix("URIs:") {
                    uris = v.trim().into();
                }
                if let Some(v) = l.strip_prefix("Suites:") {
                    sui = v.trim().into();
                }
                if let Some(v) = l.strip_prefix("Components:") {
                    comps = v.trim().into();
                }
            }
        } else {
            for l in txt.lines() {
                let t = l.trim();
                if t.starts_with("deb ") || t.starts_with("deb-src ") {
                    let p: Vec<&str> = t.split_whitespace().collect();
                    out.push(Src {
                        file: short.clone(),
                        uris: p.get(1).unwrap_or(&"").to_string(),
                        suites: p.get(2).unwrap_or(&"").to_string(),
                        comps: p[3..].join(" "),
                        deb822: false,
                    });
                }
            }
        }
    }
    out
}

/// Build the action row for one expander: Entfernen + Übernehmen
/// side by side, both queue ops (applied once by "Anwenden").
fn action_row(
    s: &Src,
    rows: &[adw::EntryRow],
    ops: &Ops,
    exp: &adw::ExpanderRow,
) -> adw::ActionRow {
    let save = adw::ActionRow::new();
    save.set_title(&gettext("Aktionen"));
    let rowbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let rm = gtk4::Button::with_label(&gettext("Entfernen"));
    rm.set_valign(gtk4::Align::Center);
    rm.add_css_class("destructive-action");
    let file_rm = s.file.clone();
    let ops_rm = ops.clone();
    let exp_rm = exp.clone();
    rm.connect_clicked(move |b| {
        ops_rm.borrow_mut().push(format!(
            "rm -f '/etc/apt/sources.list.d/{file_rm}' '/etc/apt/{file_rm}'"
        ));
        b.set_label(&gettext("✓ markiert"));
        exp_rm.set_opacity(0.4);
    });
    rowbox.append(&rm);
    let sb = gtk4::Button::with_label(&gettext("Übernehmen"));
    sb.set_valign(gtk4::Align::Center);
    sb.add_css_class("suggested-action");
    let file = s.file.clone();
    let deb822 = s.deb822;
    let ops_s = ops.clone();
    let (r0, r1, r2) = (rows[0].clone(), rows[1].clone(), rows[2].clone());
    sb.connect_clicked(move |b| {
        let content = if deb822 {
            format!(
                "Types: deb\\nURIs: {}\\nSuites: {}\\nComponents: {}\\n",
                r0.text(),
                r1.text(),
                r2.text()
            )
        } else {
            format!("deb {} {} {}\\n", r0.text(), r1.text(), r2.text())
        };
        ops_s.borrow_mut().push(format!(
            "printf '{content}' > /etc/apt/sources.list.d/{file}"
        ));
        b.set_label(&gettext("✓ gespeichert"));
    });
    rowbox.append(&sb);
    save.add_suffix(&rowbox);
    save
}

fn open_sources(parent: &impl IsA<gtk4::Widget>) {
    let ops: Ops = Rc::new(RefCell::new(Vec::new()));
    let grp = adw::PreferencesGroup::new();
    grp.set_title(&gettext("APT-Quellen"));
    for s in &read_sources() {
        let exp = adw::ExpanderRow::new();
        exp.set_title(&s.uris);
        exp.set_subtitle(&format!("{}  ·  {}", s.file, s.suites));
        let rows: Vec<adw::EntryRow> = [
            ("URIs", &s.uris),
            ("Suites", &s.suites),
            ("Components", &s.comps),
        ]
        .into_iter()
        .map(|(label, val)| {
            let row = adw::EntryRow::new();
            row.set_title(&gettext(label));
            row.set_text(val);
            row
        })
        .collect();
        for r in &rows {
            exp.add_row(r);
        }
        exp.add_row(&action_row(s, &rows, &ops, &exp));
        grp.add(&exp);
    }
    // new deb822 source — queued like the rest
    let addg = adw::PreferencesGroup::new();
    addg.set_title(&gettext("Neue Quelle (.sources)"));
    let ru = adw::EntryRow::new();
    ru.set_title("URIs");
    let rs = adw::EntryRow::new();
    rs.set_title("Suites");
    let rc = adw::EntryRow::new();
    rc.set_title("Components");
    let btn = gtk4::Button::with_label(&gettext("Hinzufügen"));
    btn.add_css_class("suggested-action");
    btn.set_margin_top(8);
    let ops_a = ops.clone();
    let (u2, s2, c2) = (ru.clone(), rs.clone(), rc.clone());
    btn.connect_clicked(move |b| {
        let uris = u2.text().trim().to_string();
        if !uris.starts_with("http") { return; }
        ops_a.borrow_mut().push(format!(
            "printf 'Types: deb\\nURIs: {}\\nSuites: {}\\nComponents: {}\\n' > /etc/apt/sources.list.d/ragnar-custom.sources",
            uris, s2.text(), c2.text()));
        b.set_label(&gettext("✓ wartet"));
    });
    addg.add(&ru);
    addg.add(&rs);
    addg.add(&rc);
    let apply = gtk4::Button::with_label(&gettext("Änderungen anwenden"));
    apply.add_css_class("suggested-action");
    apply.set_margin_top(12);
    let ops_ap = ops.clone();
    apply.connect_clicked(move |b| {
        let cmds = ops_ap.borrow().join(" && ");
        if cmds.is_empty() {
            return;
        }
        let r = std::process::Command::new("pkexec")
            .args(["sh", "-c", &cmds])
            .output();
        b.set_label(&match r {
            Ok(o) if o.status.success() => gettext("✓ Angewendet"),
            _ => gettext("Fehlgeschlagen"),
        });
    });
    let scroll = gtk4::ScrolledWindow::builder()
        .min_content_width(560)
        .min_content_height(460)
        .build();
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    inner.set_margin_start(8);
    inner.set_margin_end(8);
    inner.set_margin_top(8);
    inner.set_margin_bottom(8);
    inner.append(&grp);
    inner.append(&addg);
    inner.append(&btn);
    inner.append(&apply);
    scroll.set_child(Some(&inner));
    let tv = adw::ToolbarView::new();
    let hb = adw::HeaderBar::new();
    hb.set_show_end_title_buttons(false);
    let close = gtk4::Button::with_label(&gettext("Schließen"));
    close.add_css_class("flat");
    hb.pack_end(&close);
    tv.add_top_bar(&hb);
    tv.set_content(Some(&scroll));
    let dlg = adw::Dialog::builder()
        .title(&gettext("Quellen"))
        .child(&tv)
        .content_width(600)
        .content_height(520)
        .build();
    let d2 = dlg.clone();
    close.connect_clicked(move |_| {
        let _ = d2.close();
    });
    dlg.present(Some(parent));
}
