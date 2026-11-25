// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Processes tab — classic task-manager style: toolbar (search/filter/refresh)
//! over a sortable column table with expandable category rows
//! (Apps / Background / System) and per-app groups showing aggregate
//! usage. Right click opens the context menu; activating a process
//! row opens its details dialog. Category tints come from the
//! .cat-app/.cat-bg/.cat-sys/.cat-sub classes in the app stylesheet.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gettextrs::gettext as tr;
use gtk4::prelude::*;

use crate::proccat::{self, Category};
use crate::procfs::{Process, ProcessTable};


const C_NAME: usize = 0;
const C_PID: usize = 1;
const C_USER: usize = 2;
const C_CPU: usize = 3;
const C_MEM: usize = 4;
const C_THR: usize = 5;
const C_STAT: usize = 6;
const C_CMD: usize = 7;

/// Shared page state behind one RefCell.
struct State {
    table: ProcessTable,
    filter: String,
    mode: u32, // 0 all, 1 my processes, 2 system only
    user_filter: Option<String>,
    expanded: HashSet<String>, // "apps"|"background"|"system"|"app:<name>"
    sort_col: usize,
    sort_asc: bool,
    size_groups: Vec<gtk4::SizeGroup>,
    /// Keys + label refs of the rows currently shown — when the plan
    /// signature matches, only label texts are rewritten instead of
    /// rebuilding ~300 widgets every tick.
    live_sig: Vec<String>,
    live: Vec<Vec<gtk4::Label>>,
}

/// Build the page; returns the widget and the user-filter setter the
/// Users tab calls to jump here with a filter already applied.
pub fn page() -> (gtk4::Widget, Rc<dyn Fn(Option<String>)>) {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let state = Rc::new(RefCell::new(State {
        table: ProcessTable::new(),
        filter: String::new(),
        mode: 0,
        user_filter: None,
        expanded: HashSet::from(["apps".to_string()]),
        sort_col: C_CPU,
        sort_asc: false,
        size_groups: (0..8)
            .map(|_| gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal))
            .collect(),
        live_sig: Vec::new(),
        live: Vec::new(),
    }));

    // Toolbar: search + filter dropdown + refresh.
    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bar.set_margin_top(8);
    bar.set_margin_start(8);
    bar.set_margin_end(8);
    bar.append(&gtk4::Label::new(Some(&tr("Search:"))));
    let search = gtk4::Entry::builder()
        .hexpand(true)
        .placeholder_text(&tr("Enter process name or PID..."))
        .build();
    bar.append(&search);
    bar.append(&gtk4::Label::new(Some(&tr("Filter:"))));
    let filter = gtk4::DropDown::from_strings(&[
        &tr("All Processes"),
        &tr("My Processes"),
        &tr("System Processes"),
    ]);
    bar.append(&filter);
    let refresh = gtk4::Button::with_label(&tr("Refresh"));
    bar.append(&refresh);
    root.append(&bar);

    // Sortable header aligned to the same size groups as the rows.
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::Single);
    root.append(&header_row(&state, &list));
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    let scroll = gtk4::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .build();
    root.append(&scroll);

    search.connect_changed({
        let st = state.clone();
        let list = list.clone();
        move |e| {
            st.borrow_mut().filter = e.text().to_string();
            rebuild(&st, &list);
        }
    });
    filter.connect_selected_notify({
        let st = state.clone();
        let list = list.clone();
        move |d| {
            st.borrow_mut().mode = d.selected();
            rebuild(&st, &list);
        }
    });
    refresh.connect_clicked({
        let st = state.clone();
        let list = list.clone();
        move |_| {
            st.borrow_mut().table.refresh();
            rebuild(&st, &list);
        }
    });
    list.connect_row_activated({
        let st = state.clone();
        let list = list.clone();
        move |_, row| {
            let key = row.widget_name().to_string();
            if let Ok(pid) = key.parse::<i32>() {
                let p = st.borrow().table.rows.iter().find(|p| p.pid == pid).cloned();
                if let Some(p) = p {
                    if let Some(r) = row.root() {
                        crate::procdetail::details(&r, p);
                    }
                }
            } else {
                {
                    let mut s = st.borrow_mut();
                    if !s.expanded.remove(&key) {
                        s.expanded.insert(key);
                    }
                }
                rebuild(&st, &list);
            }
        }
    });

    rebuild(&state, &list);
    glib::timeout_add_local(crate::interval(), {
        let st = state.clone();
        let list = list.clone();
        move || {
            // Hidden tab → skip the whole /proc sweep + rebuild.
            if !list.is_visible() {
                return glib::ControlFlow::Continue;
            }
            st.borrow_mut().table.refresh();
            rebuild(&st, &list);
            glib::ControlFlow::Continue
        }
    });

    let set_user = {
        let st = state.clone();
        let list = list.clone();
        Rc::new(move |u: Option<String>| {
            st.borrow_mut().user_filter = u;
            st.borrow_mut().table.refresh();
            rebuild(&st, &list);
        }) as Rc<dyn Fn(Option<String>)>
    };
    (root.upcast(), set_user)
}

/// Clear + refill the list rows.
fn rebuild(st: &Rc<RefCell<State>>, list: &gtk4::ListBox) {
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    populate(st, list);
}

/// Sortable header — clicking a column toggles direction.
fn header_row(st: &Rc<RefCell<State>>, list: &gtk4::ListBox) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.set_margin_start(8);
    let titles = [
        tr("Name"), tr("PID"), tr("User"), tr("CPU %"),
        tr("Memory"), tr("Threads"), tr("Status"), tr("Command"),
    ];
    for (i, t) in titles.iter().enumerate() {
        let btn = gtk4::Button::with_label(t);
        btn.add_css_class("flat");
        btn.set_hexpand(i == C_CMD);
        st.borrow().size_groups[i].add_widget(&btn);
        let st = st.clone();
        let list = list.clone();
        btn.connect_clicked(move |_| {
            {
                let mut s = st.borrow_mut();
                if s.sort_col == i {
                    s.sort_asc = !s.sort_asc
                } else {
                    s.sort_col = i;
                    s.sort_asc = true;
                }
            }
            rebuild(&st, &list);
        });
        b.append(&btn);
    }
    b
}

/// One aligned table row; name cell may carry indent + chevron.
fn make_row_groups(
    groups: &[gtk4::SizeGroup],
    cells: [&str; 8],
    depth: i32,
    chevron: Option<bool>,
    tint: &str,
    bold: bool,
    name_key: &str,
) -> (gtk4::ListBoxRow, Vec<gtk4::Label>) {
    let row = gtk4::ListBoxRow::new();
    row.set_widget_name(name_key);
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.set_margin_start(8);
    let mut labels = Vec::new();
    for (i, text) in cells.iter().enumerate() {
        let cell: gtk4::Widget = if i == C_NAME {
            let nb = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
            nb.set_margin_start(depth * 16);
            if let Some(open) = chevron {
                nb.append(&gtk4::Label::new(Some(if open { "▾" } else { "▸" })));
            }
            let l = gtk4::Label::new(Some(text));
            l.set_halign(gtk4::Align::Start);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            if bold {
                l.add_css_class("heading")
            }
            nb.append(&l);
            labels.push(l);
            nb.upcast()
        } else {
            let l = gtk4::Label::new(Some(text));
            l.set_halign(gtk4::Align::Start);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            labels.push(l.clone());
            l.upcast()
        };
        groups[i].add_widget(&cell);
        b.append(&cell);
    }
    if !tint.is_empty() {
        b.add_css_class(tint)
    }
    row.set_child(Some(&b));
    (row, labels)
}


/// Sort one bucket by the active column.
fn sorted(st: &State, mut v: Vec<Process>) -> Vec<Process> {
    v.sort_by(|a, b| {
        let o = match st.sort_col {
            C_NAME => a.name.cmp(&b.name),
            C_PID => a.pid.cmp(&b.pid),
            C_USER => a.user.cmp(&b.user),
            C_CPU => a.cpu.partial_cmp(&b.cpu).unwrap_or(std::cmp::Ordering::Equal),
            C_MEM => a.rss_kb.cmp(&b.rss_kb),
            C_THR => a.threads.cmp(&b.threads),
            C_STAT => a.state.cmp(&b.state),
            _ => a.cmdline.cmp(&b.cmdline),
        };
        if st.sort_asc { o } else { o.reverse() }
    });
    v
}

/// One row in the render plan: key, cells, depth, chevron, css.
type Plan = (String, [String; 8], i32, Option<bool>, &'static str, Option<Process>);

/// Refill the table — only rewrite label texts when the row set is
/// unchanged (the cheap per-second path), full rebuild otherwise.
fn populate(st_rc: &Rc<RefCell<State>>, list: &gtk4::ListBox) {
    let plan = plan_rows(st_rc);
    let sig: Vec<String> = plan.iter().map(|p| p.0.clone()).collect();
    {
        let st = st_rc.borrow();
        if st.live_sig == sig && !plan.is_empty() && list.first_child().is_some() {
            for (labels, (_, cells, ..)) in st.live.iter().zip(&plan) {
                for (l, txt) in labels.iter().zip(cells.iter()) {
                    l.set_text(txt);
                }
            }
            return;
        }
    }
    while let Some(c) = list.first_child() {
        list.remove(&c);
    }
    let mut st = st_rc.borrow_mut();
    st.live.clear();
    st.live_sig = sig;
    for (key, cells, depth, chevron, css, proc) in &plan {
        if key.starts_with("p:") {
            let (r, labels) = proc_row(&st, cells, *depth, proc.as_ref().unwrap());
            st.live.push(labels);
            list.append(&r);
        } else {
            let (r, labels) = grp_row(&st, cells, *depth, key, css, *chevron);
            st.live.push(labels);
            list.append(&r);
        }
    }
}

/// Build the full render plan from filters + grouping.
fn plan_rows(st_rc: &Rc<RefCell<State>>) -> Vec<Plan> {
    let st = st_rc.borrow();
    let me = whoami();
    let f = st.filter.to_lowercase();
    let keep = |p: &Process| {
        let mode_ok = match st.mode {
            1 => p.user == me,
            2 => proccat::classify(p) == Category::System,
            _ => true,
        };
        let user_ok = st.user_filter.as_ref().is_none_or(|u| &p.user == u);
        let text_ok = f.is_empty()
            || p.name.to_lowercase().contains(&f)
            || p.pid.to_string().contains(&f)
            || p.cmdline.to_lowercase().contains(&f);
        mode_ok && user_ok && text_ok
    };
    let mut apps: HashMap<String, Vec<Process>> = HashMap::new();
    let mut bg = Vec::new();
    let mut sys = Vec::new();
    for p in st.table.rows.iter().filter(|p| keep(p)) {
        match proccat::classify(p) {
            Category::App => apps.entry(proccat::app_name(p)).or_default().push(p.clone()),
            Category::Background => bg.push(p.clone()),
            Category::System => sys.push(p.clone()),
        }
    }
    let mut plan: Vec<Plan> = Vec::new();
    let cat = |title: &str, n: usize| {
        let mut c: [String; 8] = Default::default();
        c[0] = format!("{title} ({n})");
        c
    };
    if st.mode != 2 {
        plan.push(("apps".into(), cat(&format!("📱 {}", tr("Apps")), apps.len()),
                   0, Some(st.expanded.contains("apps")), "cat-app", None));
        if st.expanded.contains("apps") {
            let mut names: Vec<String> = apps.keys().cloned().collect();
            names.sort();
            for n in names {
                let procs = sorted(&st, apps.remove(&n).unwrap_or_default());
                let cpu: f32 = procs.iter().map(|p| p.cpu).sum();
                let mem: u64 = procs.iter().map(|p| p.rss_kb).sum();
                let thr: i64 = procs.iter().map(|p| p.threads as i64).sum();
                let key = format!("app:{n}");
                plan.push((key.clone(), [
                    format!("{n} ({})", procs.len()), String::new(), String::new(),
                    format!("{cpu:.1}"), crate::util::kb_human(mem), thr.to_string(),
                    String::new(), String::new(),
                ], 1, Some(st.expanded.contains(&key)), "cat-sub", None));
                if st.expanded.contains(&key) {
                    for p in &procs {
                        plan.push(proc_plan(p, 2));
                    }
                }
            }
        }
        plan.push(("background".into(),
            cat(&format!("⚙️ {}", tr("Background Processes")), bg.len()),
            0, Some(st.expanded.contains("background")), "cat-bg", None));
        if st.expanded.contains("background") {
            for p in sorted(&st, bg) {
                plan.push(proc_plan(&p, 1));
            }
        }
    }
    if st.mode != 1 {
        plan.push(("system".into(),
            cat(&format!("🖥️ {}", tr("System Processes")), sys.len()),
            0, Some(st.expanded.contains("system")), "cat-sys", None));
        if st.expanded.contains("system") {
            for p in sorted(&st, sys) {
                plan.push(proc_plan(&p, 1));
            }
        }
    }
    plan
}

/// Plan entry of one process row.
fn proc_plan(p: &Process, depth: i32) -> Plan {
    (format!("p:{}", p.pid), [
        p.name.clone(), p.pid.to_string(), p.user.clone(),
        format!("{:.1}", p.cpu), crate::util::kb_human(p.rss_kb),
        p.threads.to_string(), crate::util::state_name(p.state).to_string(),
        p.cmdline.clone(),
    ], depth, None, "", Some(p.clone()))
}

/// Widget row for one process + right-click menu.
fn proc_row(st: &State, cells: &[String; 8], depth: i32, p: &Process)
        -> (gtk4::ListBoxRow, Vec<gtk4::Label>) {
    let refs: Vec<&str> = cells.iter().map(|s| s.as_str()).collect();
    let (r, labels) = make_row_groups(&st.size_groups, refs.try_into().unwrap_or([""; 8]),
        depth, None, "", false, &p.pid.to_string());
    let g = gtk4::GestureClick::new();
    g.set_button(3);
    let r2 = r.clone();
    let p2 = p.clone();
    g.connect_released(move |_, _, _, _| crate::procdetail::ctx_menu(&r2, &p2));
    r.add_controller(g);
    (r, labels)
}

/// Widget row for a category/app group — click toggles expansion.
fn grp_row(st: &State, cells: &[String; 8], depth: i32, key: &str,
           css: &str, open: Option<bool>) -> (gtk4::ListBoxRow, Vec<gtk4::Label>) {
    let refs: Vec<&str> = cells.iter().map(|s| s.as_str()).collect();
    let (r, labels) = make_row_groups(&st.size_groups, refs.try_into().unwrap_or([""; 8]),
        depth, open, css, true, key);
    (r, labels)
}


/// Current login name for the "My Processes" filter.
fn whoami() -> String {
    std::env::var("USER").unwrap_or_default()
}
