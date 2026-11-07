// Per-bar detail page for the settings window. Sections:
//   Position   — a monitor mock with four clickable edge buttons
//   Geometrie  — height / width / icon-size sliders, alignment
//   Inhalte    — applet checkboxes (exclusive per applet) plus the
//                draggable chip strip for ordering and spacers.
// Every control commits live through the bars registry.
use super::Ui;
use crate::config::{Align, BarConfig};
use gtk4::prelude::*;
use std::rc::Rc;

/// Applet id → German display name, in a fixed UI order.
pub(super) const APPLETS: &[(&str, &str)] = &[
    ("menu", "Startmenü"),
    ("tasklist", "Taskliste"),
    ("workspace", "Arbeitsflächen"),
    ("tray", "Benachrichtigungsfeld"),
    ("status", "Status"),
    ("clock", "Uhr"),
    ("notify", "Benachrichtigungen"),
    ("sensor-cpu", "CPU-Temperatur"),
    ("sensor-gpu", "GPU-Temperatur"),
    ("sensor-ram", "RAM-Temperatur"),
    ("sensor-mb", "Mainboard-Temperatur"),
];

/// Mutate the selected bar's config and push it live through the
/// registry — single commit choke-point for every control.
pub(super) fn commit(index: usize, mutate: impl FnOnce(&mut BarConfig)) {
    let mut cfg = crate::bars::bars().borrow()[index]
        .state
        .borrow()
        .cfg
        .clone();
    mutate(&mut cfg);
    crate::bars::commit(index, cfg);
}

/// The whole detail page for bar `index`.
pub fn build(ui: &Rc<Ui>, index: usize) -> gtk4::Widget {
    let cfg = crate::bars::bars().borrow()[index]
        .state
        .borrow()
        .cfg
        .clone();
    let col = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    col.set_margin_top(18);
    col.set_margin_bottom(18);
    col.set_margin_start(18);
    col.set_margin_end(18);
    col.append(&position_section(ui, index, &cfg));
    col.append(&geometry_section(index, &cfg));
    col.append(&style_section(index, &cfg));
    col.append(&content_section(ui, index, &cfg));
    col.upcast()
}

/// Section header label, left-aligned, bold.
fn head(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.set_halign(gtk4::Align::Start);
    l.add_css_class("heading");
    l
}

// ---------- Position ----------

/// A monitor mock whose four edges are toggle buttons — clicking an
/// edge docks the bar there, no dropdown needed.
fn position_section(ui: &Rc<Ui>, index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let sec = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    sec.append(&head(&crate::i18n::tr("Position")));

    let grid = gtk4::Grid::new();
    grid.set_halign(gtk4::Align::Center);
    grid.set_row_spacing(6);
    grid.set_column_spacing(6);
    let screen = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    screen.add_css_class("screen-mock");
    screen.set_size_request(150, 85);
    grid.attach(&screen, 1, 1, 1, 1);

    // (column, row, config edge value)
    for (x, y, edge) in [
        (1, 0, "top"),
        (0, 1, "left"),
        (2, 1, "right"),
        (1, 2, "bottom"),
    ] {
        let b = gtk4::ToggleButton::new();
        b.add_css_class("edge-btn");
        b.set_size_request(64, 30);
        b.set_active(cfg.edge == edge);
        b.set_tooltip_text(Some(match edge {
            "top" => "Oben",
            "left" => "Links",
            "right" => "Rechts",
            _ => "Unten",
        }));
        let u = ui.clone();
        b.connect_toggled(move |t| {
            if t.is_active() {
                commit(index, |c| c.edge = edge.into());
                super::rebuild_all(&u);
            }
        });
        grid.attach(&b, x, y, 1, 1);
    }

    // Radio behaviour: keep exactly one edge active.
    let mut leader: Option<gtk4::ToggleButton> = None;
    let mut w = grid.first_child();
    while let Some(child) = w {
        if let Ok(t) = child.clone().downcast::<gtk4::ToggleButton>() {
            if let Some(l) = &leader {
                t.set_group(Some(l));
            } else {
                leader = Some(t);
            }
        }
        w = child.next_sibling();
    }
    sec.append(&grid);
    sec.upcast()
}

// ---------- Geometry ----------

/// Sliders for height, width and icon size, plus the alignment
/// toggles whose labels follow the bar's orientation.
fn geometry_section(index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let sec = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    sec.append(&head(&crate::i18n::tr("Geometrie")));
    let horizontal = cfg.edge == "top" || cfg.edge == "bottom";

    sec.append(&slider(
        index,
        &crate::i18n::tr("Höhe"),
        24.0,
        96.0,
        cfg.height as f64,
        |c, v| c.height = v as i32,
    ));
    sec.append(&slider(
        index,
        &crate::i18n::tr(if horizontal { "Breite %" } else { "Höhe %" }),
        30.0,
        100.0,
        cfg.width_percent as f64,
        |c, v| c.width_percent = v as i32,
    ));
    sec.append(&slider(
        index,
        &crate::i18n::tr("Icon-Größe"),
        12.0,
        48.0,
        cfg.icon_size as f64,
        |c, v| c.icon_size = v as i32,
    ));

    // Alignment toggles — labels follow the long axis' direction.
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let names: [&str; 3] = if horizontal {
        ["Links", "Mitte", "Rechts"]
    } else {
        ["Oben", "Mitte", "Unten"]
    };
    let mut leader: Option<gtk4::ToggleButton> = None;
    for (i, (name, al)) in names
        .iter()
        .zip([Align::Left, Align::Center, Align::Right])
        .enumerate()
    {
        let t = gtk4::ToggleButton::with_label(&crate::i18n::tr(name));
        t.set_active(
            cfg.align == al || (i == 1 && cfg.align != Align::Left && cfg.align != Align::Right),
        );
        if let Some(l) = &leader {
            t.set_group(Some(l));
        } else {
            leader = Some(t.clone());
        }
        t.connect_toggled(move |t| {
            if t.is_active() {
                commit(index, |c| c.align = al);
            }
        });
        row.append(&t);
    }
    let lrow = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    lrow.append(&gtk4::Label::new(Some(&crate::i18n::tr("Ausrichtung"))));
    lrow.append(&row);
    row.set_sensitive(cfg.width_percent < 100);
    sec.append(&lrow);
    sec.upcast()
}

/// One labelled slider that commits on every change.
fn slider(
    index: usize,
    name: &str,
    min: f64,
    max: f64,
    value: f64,
    mutate: impl Fn(&mut BarConfig, f64) + 'static,
) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let l = gtk4::Label::new(Some(name));
    l.set_size_request(90, -1);
    l.set_halign(gtk4::Align::Start);
    row.append(&l);
    let s = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, 1.0);
    s.set_value(value);
    s.set_hexpand(true);
    s.set_draw_value(true);
    s.connect_value_changed(move |s| {
        commit(index, |c| mutate(c, s.value()));
    });
    row.append(&s);
    row.upcast()
}

// ---------- Style ----------

/// Opacity slider plus a colour picker — per bar, live.
fn style_section(index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let sec = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    sec.append(&head(&crate::i18n::tr("Darstellung")));

    sec.append(&slider(
        index,
        &crate::i18n::tr("Transparenz"),
        10.0,
        100.0,
        cfg.opacity as f64,
        |c, v| c.opacity = v as i32,
    ));

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(&crate::i18n::tr("Farbe"))));
    let dialog = gtk4::ColorDialog::builder().with_alpha(true).build();
    let pick = gtk4::ColorDialogButton::new(Some(dialog));
    let rgba = gtk4::gdk::RGBA::parse(&cfg.color)
        .unwrap_or_else(|_| gtk4::gdk::RGBA::new(0.15, 0.15, 0.15, 1.0));
    pick.set_rgba(&rgba);
    pick.connect_rgba_notify(move |b| {
        let c = b.rgba();
        let hex = format!(
            "#{:02x}{:02x}{:02x}",
            (c.red() * 255.0) as u8,
            (c.green() * 255.0) as u8,
            (c.blue() * 255.0) as u8
        );
        commit(index, |c| c.color = hex);
    });
    row.append(&pick);
    let reset = gtk4::Button::with_label(&crate::i18n::tr("Standard"));
    reset.connect_clicked(move |_| commit(index, |c| c.color.clear()));
    row.append(&reset);
    // Monitor pick — connector names from the live display.
    let mrow = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    mrow.append(&gtk4::Label::new(Some(&crate::i18n::tr("Monitor"))));
    if let Some(disp) = gtk4::gdk::Display::default() {
        let mons = disp.monitors();
        let mut names = vec![crate::i18n::tr("Automatisch")];
        for i in 0..mons.n_items() {
            if let Some(m) = mons
                .item(i)
                .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
            {
                names.push(
                    m.connector()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| format!("Monitor {i}")),
                );
            }
        }
        let dd =
            gtk4::DropDown::from_strings(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let cur = names.iter().position(|n| *n == cfg.monitor).unwrap_or(0);
        dd.set_selected(cur as u32);
        let names2 = names.clone();
        dd.connect_selected_notify(move |d| {
            let name = names2[d.selected() as usize].clone();
            commit(index, move |c| {
                c.monitor = if name == "Automatisch" {
                    String::new()
                } else {
                    name
                }
            });
        });
        mrow.append(&dd);
    }
    sec.append(&mrow);
    sec.append(&row);
    sec.upcast()
}

// ---------- Content ----------

/// Applet checkboxes (an applet can only live on ONE bar — checking
/// it here removes it from every other bar) plus the draggable chip
/// strip that sets the order along the bar.
fn content_section(ui: &Rc<Ui>, index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let sec = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    sec.append(&head(&crate::i18n::tr("Inhalte")));

    // Checkbox per known applet — toggling updates the layout list.
    let checks = gtk4::FlowBox::new();
    checks.set_max_children_per_line(3);
    checks.set_selection_mode(gtk4::SelectionMode::None);
    for (id, name) in APPLETS {
        let cb = gtk4::CheckButton::with_label(name);
        cb.set_active(cfg.layout.iter().any(|i| i == id));
        let u = ui.clone();
        cb.connect_toggled(move |cb| {
            if cb.is_active() {
                // Exclusivity: pull the applet out of every other bar.
                crate::bars::take_applet(id, index);
                commit(index, |c| {
                    if !c.layout.iter().any(|i| i == id) {
                        // Insert before the clock — it stays the
                        // rightmost applet by convention.
                        match c.layout.iter().position(|i| i == "clock") {
                            Some(p) => c.layout.insert(p, id.to_string()),
                            None => c.layout.push(id.to_string()),
                        }
                    }
                });
            } else {
                commit(index, |c| c.layout.retain(|i| i != id));
            }
            super::rebuild_all(&u);
        });
        checks.insert(&cb, -1);
    }
    sec.append(&checks);

    // Workspace controls — only meaningful while the applet lives
    // on this bar: the global count rewrites the session
    // compositor's config (labwc ignores ext-workspace's
    // create_workspace), the cap only limits this bar's buttons.
    if cfg.layout.iter().any(|i| i == "workspace") {
        sec.append(&super::workspace_ctl::row(index, cfg));
    }

    // Chip strip — drag to reorder, spacer chips click-edit width.
    let strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    strip.add_css_class("layout-preview");
    strip.set_size_request(-1, 34);
    for (i, item) in cfg.layout.iter().enumerate() {
        strip.append(&super::bar_chip::chip(ui, index, item, i));
    }
    let spacer = gtk4::Button::with_label(&crate::i18n::tr("+ Füllraum"));
    let u = ui.clone();
    spacer.connect_clicked(move |_| {
        commit(index, |c| c.layout.push("spacer".into()));
        super::rebuild_all(&u);
    });
    sec.append(&strip);
    sec.append(&spacer);
    sec.upcast()
}
