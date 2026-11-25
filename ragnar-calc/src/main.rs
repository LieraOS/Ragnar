// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-calc — Ragnar calculator.
//
// GTK4 window with two layouts sharing one entry: the standard
// grid (arithmetic, percent, square/root/reciprocal) plus a
// scientific block that the "Erweitert" header toggle reveals.
// Buttons only edit the entry text; evaluation happens through
// parser::eval on "=" or Enter, so typed expressions and button
// input behave identically.

mod i18n;
mod parser;

use gtk4::prelude::*;
use i18n::tr;
use parser::AngleMode;
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    i18n::init();
    let app = gtk4::Application::new(
        Some("org.lieraos.ragnar.Calc"),
        gtk4::gio::ApplicationFlags::FLAGS_NONE,
    );
    app.connect_activate(build_ui);
    // Empty argv: nothing may be mistaken for a file to open.
    app.run_with_args(&[] as &[&str]);
}

/// What a button does to the entry when clicked.
enum Key {
    /// Append literal text (digits, operators, constants).
    Insert(&'static str),
    /// Wrap the whole entry: "name(entry)", or insert "name("
    /// when the entry is empty.
    Func(&'static str),
    /// Empty the entry.
    Clear,
    /// Delete the last character.
    Back,
    /// Negate a numeric entry, or prefix "-(" for expressions.
    Negate,
    /// Evaluate the entry through the parser.
    Eval,
}

/// Shared state handed to every button.
struct Ctx {
    entry: gtk4::Entry,
    mode: Rc<Cell<AngleMode>>,
}

fn build_ui(app: &gtk4::Application) {
    let mode = Rc::new(Cell::new(AngleMode::Degrees));

    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some(&tr("Rechner")));
    window.set_default_size(290, 400);
    // resizable: the scientific block needs the window to grow;
    // a fixed size would squeeze the standard keypad out of view.

    let header = gtk4::HeaderBar::new();
    let sci_toggle = gtk4::ToggleButton::with_label(&tr("Erweitert"));
    let deg_toggle = gtk4::ToggleButton::with_label("DEG");
    deg_toggle.set_active(true);
    deg_toggle.set_visible(false);
    header.pack_end(&sci_toggle);
    header.pack_start(&deg_toggle);
    window.set_titlebar(Some(&header));

    let entry = gtk4::Entry::new();
    entry.add_css_class("calc-display");
    gtk4::prelude::EditableExt::set_alignment(&entry, 1.0);
    entry.set_placeholder_text(Some("0"));

    let ctx = Rc::new(Ctx {
        entry: entry.clone(),
        mode: mode.clone(),
    });

    let sci = build_sci_grid(&ctx);
    sci.set_visible(false);
    let grids = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    grids.append(&sci);
    grids.append(&build_std_grid(&ctx));

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    root.set_margin_top(6);
    root.set_margin_bottom(6);
    root.set_margin_start(6);
    root.set_margin_end(6);
    root.append(&entry);
    root.append(&grids);
    window.set_child(Some(&root));

    load_css();

    // Toggles: "Erweitert" reveals the sci block + unit switch and
    // widens the window so both grids stay fully visible.
    let deg = deg_toggle.clone();
    let win = window.clone();
    sci_toggle.connect_toggled(move |b| {
        sci.set_visible(b.is_active());
        deg.set_visible(b.is_active());
        win.set_default_size(if b.is_active() { 560 } else { 290 }, 400);
    });
    deg_toggle.connect_toggled(move |b| {
        b.set_label(if b.is_active() { "DEG" } else { "RAD" });
        mode.set(if b.is_active() {
            AngleMode::Degrees
        } else {
            AngleMode::Radians
        });
    });
    entry.connect_activate(move |e| evaluate(e, ctx.mode.get()));

    window.present();
}

/// Display font and key sizing.
fn load_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_data(
        "entry.calc-display { font-size: 24px; font-family: monospace; }\
         button.calc-key { min-width: 52px; min-height: 44px; }\
         button.calc-fn { font-size: 13px; }",
    );
    if let Some(d) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &d,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Append text to the entry and keep the cursor at the end.
fn insert(entry: &gtk4::Entry, text: &str) {
    let mut pos = entry.position();
    entry.insert_text(text, &mut pos);
    entry.set_position(-1);
}

/// Wrap the whole entry in `name(...)`, or insert `name(` when
/// empty — scientific function keys.
fn func_wrap(entry: &gtk4::Entry, name: &str) {
    let t = entry.text();
    entry.set_text(&if t.is_empty() {
        format!("{name}(")
    } else {
        format!("{name}({t})")
    });
    entry.set_position(-1);
}

/// Evaluate the entry and write the result back (or "Fehler").
fn evaluate(entry: &gtk4::Entry, mode: AngleMode) {
    let text = entry.text();
    if text.trim().is_empty() {
        return;
    }
    match parser::eval(text.as_str(), mode) {
        Ok(v) => entry.set_text(&format_result(v)),
        Err(_) => entry.set_text(&tr("Fehler")),
    }
    entry.set_position(-1);
}

/// Integers print plainly; fractions keep up to 10 decimals with
/// trailing zeros trimmed.
fn format_result(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{v:.0}");
    }
    format!("{v:.10}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Create one button in `grid`, wired to `ctx`.
fn add(grid: &gtk4::Grid, ctx: &Rc<Ctx>, col: i32, row: i32, label: &str, key: Key, small: bool) {
    let b = gtk4::Button::with_label(label);
    b.add_css_class("calc-key");
    if small {
        b.add_css_class("calc-fn");
    }
    if label == "=" {
        b.add_css_class("suggested-action");
    }
    match key {
        Key::Insert(t) => {
            let e = ctx.entry.clone();
            b.connect_clicked(move |_| insert(&e, t));
        }
        Key::Func(name) => {
            let e = ctx.entry.clone();
            b.connect_clicked(move |_| func_wrap(&e, name));
        }
        Key::Clear => {
            let e = ctx.entry.clone();
            b.connect_clicked(move |_| e.set_text(""));
        }
        Key::Back => {
            let e = ctx.entry.clone();
            b.connect_clicked(move |_| {
                let pos = e.position();
                if pos > 0 {
                    e.delete_text(pos - 1, pos);
                    e.set_position(pos - 1);
                }
            });
        }
        Key::Negate => {
            let e = ctx.entry.clone();
            b.connect_clicked(move |_| {
                let t = e.text();
                if let Ok(v) = t.parse::<f64>() {
                    e.set_text(&format_result(-v));
                } else if !t.is_empty() {
                    e.set_text(&format!("-({t})"));
                }
                e.set_position(-1);
            });
        }
        Key::Eval => {
            let e = ctx.entry.clone();
            let m = ctx.mode.clone();
            b.connect_clicked(move |_| evaluate(&e, m.get()));
        }
    }
    grid.attach(&b, col, row, 1, 1);
}

/// The always-visible standard keypad (4 columns).
fn build_std_grid(ctx: &Rc<Ctx>) -> gtk4::Grid {
    let g = gtk4::Grid::builder().row_spacing(4).column_spacing(4).build();
    let rows: [[(&str, Key); 4]; 6] = [
        [("%", Key::Insert("%")), ("CE", Key::Clear), ("C", Key::Clear), ("⌫", Key::Back)],
        [("1/x", Key::Func("1/")), ("x²", Key::Insert("^2")), ("√", Key::Func("sqrt")), ("÷", Key::Insert("÷"))],
        [("7", Key::Insert("7")), ("8", Key::Insert("8")), ("9", Key::Insert("9")), ("×", Key::Insert("×"))],
        [("4", Key::Insert("4")), ("5", Key::Insert("5")), ("6", Key::Insert("6")), ("−", Key::Insert("−"))],
        [("1", Key::Insert("1")), ("2", Key::Insert("2")), ("3", Key::Insert("3")), ("+", Key::Insert("+"))],
        [("±", Key::Negate), ("0", Key::Insert("0")), (".", Key::Insert(".")), ("=", Key::Eval)],
    ];
    for (row, cols) in rows.into_iter().enumerate() {
        for (col, (label, key)) in cols.into_iter().enumerate() {
            add(&g, ctx, col as i32, row as i32, label, key, false);
        }
    }
    g
}

/// The scientific block (3 columns), hidden until "Erweitert".
fn build_sci_grid(ctx: &Rc<Ctx>) -> gtk4::Grid {
    let g = gtk4::Grid::builder().row_spacing(4).column_spacing(4).build();
    let cells: [(&str, Key); 18] = [
        ("(", Key::Insert("(")),
        (")", Key::Insert(")")),
        ("|x|", Key::Func("abs")),
        ("sin", Key::Func("sin")),
        ("cos", Key::Func("cos")),
        ("tan", Key::Func("tan")),
        ("asin", Key::Func("asin")),
        ("acos", Key::Func("acos")),
        ("atan", Key::Func("atan")),
        ("ln", Key::Func("ln")),
        ("log", Key::Func("log")),
        ("eˣ", Key::Func("exp")),
        ("xʸ", Key::Insert("^")),
        ("10ˣ", Key::Func("10^")),
        ("n!", Key::Insert("!")),
        ("π", Key::Insert("π")),
        ("e", Key::Insert("e")),
        ("mod", Key::Insert(" mod ")),
    ];
    for (i, (label, key)) in cells.into_iter().enumerate() {
        add(&g, ctx, i as i32 % 3, i as i32 / 3, label, key, true);
    }
    g
}
