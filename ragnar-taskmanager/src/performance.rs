// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Performance tab — classic task-manager layout: left selector column
//! (CPU/Memory/Disk/Network/GPU) and a content area per category.
//! CPU shows one vertical tile bar per core plus model/frequency
//! info; the other categories show live rate rows. All counters come
//! from sysstat; bars are cairo DrawingAreas with 60 s history.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gettextrs::gettext as tr;
use gtk4::prelude::*;

use crate::sysstat;

const HISTORY: usize = 60;

use crate::tiles::TileSet;

/// Build the performance page: selector column + content stack.
pub fn page() -> gtk4::Widget {
    let root = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let sel = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    sel.set_margin_top(8); sel.set_margin_start(8); sel.set_margin_bottom(8);
    sel.set_size_request(140, -1);
    let stack = gtk4::Stack::new();
    stack.set_hexpand(true);

    let (cpu_w, cpu_tiles, cpu_info) = cpu_page();
    let (mem_w, mem_live) = mem_page();
    let (disk_w, disk_box, disk_g, disk_tiles, prev_disk) = rates_page(&tr("Disk"));
    let (net_w, net_box, net_g, net_tiles, prev_net) = rates_page(&tr("Network"));
    let (gpu_w, gpu_box, gpu_g, gpu_tiles) = gpu_page();
    for (id, w) in [("cpu", cpu_w), ("mem", mem_w), ("disk", disk_w),
                    ("net", net_w), ("gpu", gpu_w)] {
        stack.add_named(&w, Some(id));
    }

    let mut first: Option<gtk4::ToggleButton> = None;
    for (id, label) in [("cpu", tr("CPU")), ("mem", tr("Memory")),
                        ("disk", tr("Disk")), ("net", tr("Network")),
                        ("gpu", tr("GPU"))] {
        let b = gtk4::ToggleButton::with_label(&label);
        if let Some(f) = &first { b.set_group(Some(f)) } else {
            b.set_active(true); first = Some(b.clone())
        }
        let stack = stack.clone();
        b.connect_toggled(move |b| if b.is_active() { stack.set_visible_child_name(id) });
        sel.append(&b);
    }
    root.append(&sel);
    root.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    root.append(&stack);

    // Live state for the shared tick.
    let sampler = Rc::new(RefCell::new(sysstat::CpuSampler::default()));
    let vis = root.clone();
    glib::timeout_add_local(crate::interval(), move || {
        if !vis.is_visible() {
            return glib::ControlFlow::Continue;
        }
        let cores = sampler.borrow_mut().sample();
        for (i, v) in cores.iter().enumerate() {
            cpu_tiles.set(&format!("{} {i}", tr("Core")), *v as f64,
                          &format!("{:.1}%", *v * 100.0));
        }
        let m = sysstat::meminfo();
        mem_live.0.set_fraction(if m.total > 0 {
            (m.total - m.avail) as f64 / m.total as f64 } else { 0.0 });
        mem_live.1.set_text(&format!("{} / {}",
            crate::util::kb_human(m.total - m.avail),
            crate::util::kb_human(m.total)));
        let su = m.swap_total.saturating_sub(m.swap_free);
        mem_live.2.set_fraction(if m.swap_total > 0 {
            su as f64 / m.swap_total as f64 } else { 0.0 });
        mem_live.3.set_text(&format!("{} / {}",
            crate::util::kb_human(su),
            crate::util::kb_human(m.swap_total)));
        cpu_info.0.set_text(&cpu_model_text());
        let ram_frac = if m.total > 0 {
            (m.total - m.avail) as f64 / m.total as f64 } else { 0.0 };
        let swap_frac = if m.swap_total > 0 {
            su as f64 / m.swap_total as f64 } else { 0.0 };
        mem_live.5.set(&tr("RAM"), ram_frac, &format!("{:.0}%", ram_frac * 100.0));
        mem_live.5.set(&tr("Swap"), swap_frac, &format!("{:.0}%", swap_frac * 100.0));
        mem_live.4.push(if m.total > 0 {
            (m.total - m.avail) as f64 / m.total as f64 } else { 0.0 }, 1.0);
        let net = sysstat::net_io();
        let disk = sysstat::disk_io();
        let pn = prev_net.borrow();
        let pd = prev_disk.borrow();
        net_g.push(net.iter().map(|(n, a, b)| {
            let (pa, pb) = pn.iter().find(|(x, _, _)| x == n)
                .map(|(_, x, y)| (*x, *y)).unwrap_or((*a, *b));
            (a - pa + b - pb) as f64
        }).sum::<f64>(), 10.0 * 1_048_576.0);
        disk_g.push(disk.iter().map(|(n, a, b)| {
            let (pa, pb) = pd.iter().find(|(x, _, _)| x == n)
                .map(|(_, x, y)| (*x, *y)).unwrap_or((*a, *b));
            (a - pa + b - pb) as f64
        }).sum::<f64>(), 100.0 * 1_048_576.0);
        drop(pn); drop(pd);
        net_tiles.retain(&net.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>());
        disk_tiles.retain(&disk.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>());
        for (n, a, b) in &net {
            let (pa, pb) = prev_net.borrow().iter().find(|(x, _, _)| x == n)
                .map(|(_, x, y)| (*x, *y)).unwrap_or((*a, *b));
            let rate = (a - pa + b - pb) as f64;
            net_tiles.set(n, (rate / (10.0 * 1_048_576.0)).min(1.0),
                          &format!("↓{} ↑{}", human_rate(a - pa), human_rate(b - pb)));
        }
        for (n, a, b) in &disk {
            let (pa, pb) = prev_disk.borrow().iter().find(|(x, _, _)| x == n)
                .map(|(_, x, y)| (*x, *y)).unwrap_or((*a, *b));
            let rate = (a - pa + b - pb) as f64;
            disk_tiles.set(n, (rate / (100.0 * 1_048_576.0)).min(1.0),
                           &format!("↓{} ↑{}", human_rate(a - pa), human_rate(b - pb)));
        }
        rates(&net_box, &net, &mut prev_net.borrow_mut());
        rates(&disk_box, &disk, &mut prev_disk.borrow_mut());
        let gs = sysstat::gpus();
        gpu_g.push(gs.first().map(|g| g.busy as f64).unwrap_or(0.0), 1.0);
        gpu_tiles.retain(&gs.iter().map(|g| g.name.clone()).collect::<Vec<_>>());
        for g in &gs {
            gpu_tiles.set(&g.name, g.busy as f64, &match g.temp {
                Some(t) => format!("{:.0}%, {:.0}°C", g.busy * 100.0, t),
                None => format!("{:.0}%", g.busy * 100.0),
            });
        }
        gpus_rows(&gpu_box, &gs);
        glib::ControlFlow::Continue
    });
    root.upcast()
}

/// CPU page: grid of vertical per-core tiles + info card.
fn cpu_page() -> (gtk4::Widget, TileSet, (gtk4::Label,)) {
    let scroll = gtk4::ScrolledWindow::new();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    b.set_margin_top(16); b.set_margin_start(16);
    b.set_margin_end(16); b.set_margin_bottom(16);
    let head = gtk4::Label::builder().label(&tr("CPU")).halign(gtk4::Align::Start).build();
    head.add_css_class("title-2");
    b.append(&head);

    let tiles = TileSet::new();
    b.append(&tiles.widget);
    for i in 0..crate::procfs::num_cpus() {
        tiles.set(&format!("{} {i}", tr("Core")), 0.0, "0.0%");
    }

    let info = gtk4::Label::builder().halign(gtk4::Align::Start)
        .label(&cpu_model_text()).build();
    info.add_css_class("card"); info.set_margin_top(8);
    info.set_margin_start(8); info.set_margin_end(8);
    b.append(&info);
    scroll.set_child(Some(&b));
    (scroll.upcast(), tiles, (info,))
}


/// CPU model/cores/frequency card text.
fn cpu_model_text() -> String {
    let s = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = s.lines().find_map(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string()).unwrap_or_default();
    let cur = std::fs::read_to_string(
        "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        .ok().and_then(|v| v.trim().parse::<f64>().ok())
        .map(|khz| format!("{:.2} GHz", khz / 1e6)).unwrap_or_default();
    let max = std::fs::read_to_string(
        "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .ok().and_then(|v| v.trim().parse::<f64>().ok())
        .map(|khz| format!("{:.2} GHz", khz / 1e6)).unwrap_or_default();
    format!("{}: {}\n{}: {}\n{}: {}\n{}: {}",
        tr("Model"), model, tr("Cores"), crate::procfs::num_cpus(),
        tr("Current"), cur, tr("Max"), max)
}

/// Memory page: RAM + swap meters.
fn mem_page() -> (gtk4::Widget, (gtk4::ProgressBar, gtk4::Label,
                                 gtk4::ProgressBar, gtk4::Label, Graph, TileSet)) {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    b.set_margin_top(16); b.set_margin_start(16);
    b.set_margin_end(16); b.set_margin_bottom(16);
    let head = gtk4::Label::builder().label(&tr("Memory")).halign(gtk4::Align::Start).build();
    head.add_css_class("title-2");
    b.append(&head);
    let tiles = TileSet::new();
    b.append(&tiles.widget);
    tiles.set(&tr("RAM"), 0.0, "0%");
    tiles.set(&tr("Swap"), 0.0, "0%");
    let g = graph();
    b.append(&g.area);
    let (mb, ml) = meter(&b, &tr("RAM"));
    let (sb, sl) = meter(&b, &tr("Swap"));
    (b.upcast(), (mb, ml, sb, sl, g, tiles))
}

/// One labelled meter row.
fn meter(parent: &gtk4::Box, name: &str) -> (gtk4::ProgressBar, gtk4::Label) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::builder().label(name).width_chars(6)
        .halign(gtk4::Align::Start).build());
    let bar = gtk4::ProgressBar::builder().hexpand(true).build();
    let val = gtk4::Label::new(Some("—"));
    row.append(&bar); row.append(&val);
    parent.append(&row);
    (bar, val)
}

/// Generic per-device rate page; returns (widget, rows box, prev).
fn rates_page(title: &str)
    -> (gtk4::Widget, gtk4::Box, Graph, TileSet, Rc<RefCell<Vec<(String, u64, u64)>>>) {
    let scroll = gtk4::ScrolledWindow::new();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    b.set_margin_top(16); b.set_margin_start(16);
    b.set_margin_end(16); b.set_margin_bottom(16);
    let h = gtk4::Label::builder().label(title).halign(gtk4::Align::Start).build();
    h.add_css_class("title-2");
    b.append(&h);
    let tiles = TileSet::new();
    b.append(&tiles.widget);
    let g = graph();
    b.append(&g.area);
    let rows = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    b.append(&rows);
    scroll.set_child(Some(&b));
    (scroll.upcast(), rows, g, tiles, Rc::new(RefCell::new(Vec::new())))
}

/// Rebuild a device rate list from byte-counter deltas.
fn rates(container: &gtk4::Box, now: &[(String, u64, u64)],
         prev: &mut Vec<(String, u64, u64)>) {
    while let Some(c) = container.first_child() { container.remove(&c); }
    for (name, a, b) in now {
        let (pa, pb) = prev.iter().find(|(n, _, _)| n == name)
            .map(|(_, x, y)| (*x, *y)).unwrap_or((*a, *b));
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row.append(&gtk4::Label::builder().label(name).hexpand(true)
            .halign(gtk4::Align::Start).build());
        row.append(&gtk4::Label::new(Some(&format!(
            "↓ {} B/s | ↑ {} B/s", human_rate(a - pa), human_rate(b - pb)))));
        container.append(&row);
    }
    *prev = now.to_vec();
}

/// Bytes → human rate.
fn human_rate(b: u64) -> String {
    let mb = b as f64 / 1_048_576.0;
    if mb >= 1.0 { format!("{:.1} M", mb) } else { format!("{:.0} K", b as f64 / 1024.0) }
}

/// GPU page: one meter row per detected adapter.
fn gpu_page() -> (gtk4::Widget, gtk4::Box, Graph, TileSet) {
    let scroll = gtk4::ScrolledWindow::new();
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    b.set_margin_top(16); b.set_margin_start(16);
    b.set_margin_end(16); b.set_margin_bottom(16);
    let h = gtk4::Label::builder().label(&tr("GPU")).halign(gtk4::Align::Start).build();
    h.add_css_class("title-2");
    b.append(&h);
    let tiles = TileSet::new();
    b.append(&tiles.widget);
    let g = graph();
    b.append(&g.area);
    let rows = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    b.append(&rows);
    scroll.set_child(Some(&b));
    (scroll.upcast(), rows, g, tiles)
}

/// Rebuild GPU rows from a sysfs/smi sample.
fn gpus_rows(container: &gtk4::Box, gs: &[sysstat::Gpu]) {
    while let Some(c) = container.first_child() { container.remove(&c); }
    for g in gs {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row.append(&gtk4::Label::builder().label(&g.name).hexpand(true)
            .halign(gtk4::Align::Start).build());
        row.append(&gtk4::ProgressBar::builder()
            .fraction(g.busy as f64).hexpand(true)
            .valign(gtk4::Align::Center).build());
        row.append(&gtk4::Label::new(Some(&match g.temp {
            Some(t) => format!("{:.0} %, {:.0} °C", g.busy * 100.0, t),
            None => format!("{:.0} %", g.busy * 100.0),
        })));
        container.append(&row);
    }
    if container.first_child().is_none() {
        let l = gtk4::Label::new(Some(&tr("No GPU detected")));
        l.add_css_class("dim-label");
        container.append(&l);
    }
}

/// Rolling history + a line-graph DrawingArea, like the original's
/// per-category usage graphs.
#[derive(Clone)]
struct Graph { area: gtk4::DrawingArea, hist: Rc<RefCell<VecDeque<f64>>> }
impl Graph {
    /// Push one sample (already normalised by `scale` to 0..1) and repaint.
    fn push(&self, v: f64, scale: f64) {
        let mut h = self.hist.borrow_mut();
        h.push_back(v / scale.max(1.0));
        if h.len() > HISTORY {
            h.pop_front();
        }
        self.area.queue_draw();
    }
}

/// New graph widget of a fixed height.
fn graph() -> Graph {
    let area = gtk4::DrawingArea::builder()
        .height_request(110)
        .hexpand(true)
        .build();
    let hist = Rc::new(RefCell::new(VecDeque::<f64>::new()));
    area.set_draw_func({
        let hist = hist.clone();
        move |_, cr, w, h| {
            let hst = hist.borrow();
            let n = hst.len();
            if n < 2 {
                return;
            }
            let (w, h) = (w as f64, h as f64);
            cr.set_source_rgba(0.3, 0.6, 0.95, 0.9);
            cr.set_line_width(1.5);
            for (i, v) in hst.iter().enumerate() {
                let x = i as f64 / (HISTORY - 1) as f64 * w;
                let y = h - v.clamp(0.0, 1.0) * (h - 4.0) - 2.0;
                if i == 0 {
                    cr.move_to(x, y)
                } else {
                    cr.line_to(x, y)
                }
            }
            let _ = cr.stroke();
        }
    });
    area.add_css_class("card");
    Graph { area, hist }
}
