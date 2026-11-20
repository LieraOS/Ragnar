// Display-preferences section of the clock settings dialog —
// everything that only changes how the *bar label* looks: weekday
// prefix, text colour, font weight and date format. None of these
// touch the system clock, so this block deliberately lives apart
// from the timedatectl-backed settings: changing them applies
// live through clock_cfg and never asks for a password.
use gtk4::prelude::*;
use crate::applets::clock_cfg as cfg;
use crate::i18n::tr;

/// Build the preferences grid — callers append it to the dialog
/// below the system-time controls.
pub fn section() -> gtk4::Widget {
    let grid = gtk4::Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(12);

    // Weekday prefix in the bar label — off, abbreviated or full
    // name.
    let wd = gtk4::DropDown::new(
        Some(gtk4::StringList::new(&[
            &tr("Aus"),
            &tr("Abgekürzt"),
            &tr("Vollständig"),
        ])),
        gtk4::Expression::NONE,
    );
    wd.set_selected(match cfg::weekday_saved() {
        cfg::Weekday::Off => 0,
        cfg::Weekday::Abbr => 1,
        cfg::Weekday::Full => 2,
    });
    wd.connect_selected_notify(|d| {
        cfg::set_weekday(match d.selected() {
            1 => cfg::Weekday::Abbr,
            2 => cfg::Weekday::Full,
            _ => cfg::Weekday::Off,
        });
    });
    attach(&grid, &tr("Wochentag"), &wd, 0);

    // Font weight — Normal renders through set_text; the heavier
    // weights go through a Pango weight span.
    let wt = gtk4::DropDown::new(
        Some(gtk4::StringList::new(&[
            &tr("Normal"),
            &tr("Halbfett"),
            &tr("Fett"),
        ])),
        gtk4::Expression::NONE,
    );
    wt.set_selected(match cfg::weight_saved() {
        cfg::Weight::Normal => 0,
        cfg::Weight::SemiBold => 1,
        cfg::Weight::Bold => 2,
    });
    wt.connect_selected_notify(|d| {
        cfg::set_weight(match d.selected() {
            1 => cfg::Weight::SemiBold,
            2 => cfg::Weight::Bold,
            _ => cfg::Weight::Normal,
        });
    });
    attach(&grid, &tr("Schriftstärke"), &wt, 1);

    // Label colour — custom colour helps readability when the bar
    // is transparent over a light wallpaper; "Standard" restores
    // the theme colour.
    let colour = gtk4::ColorDialogButton::new(Some(
        gtk4::ColorDialog::builder().with_alpha(false).build(),
    ));
    if let Some(hex) = cfg::color_saved() {
        if let Ok(c) = gtk4::gdk::RGBA::parse(&hex) {
            colour.set_rgba(&c);
        }
    }
    colour.connect_rgba_notify(|b| {
        let c = b.rgba();
        cfg::set_color(Some(format!(
            "#{:02x}{:02x}{:02x}",
            (c.red() * 255.0) as u8,
            (c.green() * 255.0) as u8,
            (c.blue() * 255.0) as u8
        )));
    });
    let reset = gtk4::Button::with_label(&tr("Standard"));
    reset.set_has_frame(false);
    reset.connect_clicked(|_| cfg::set_color(None));
    let colour_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    colour_row.append(&colour);
    colour_row.append(&reset);
    attach(&grid, &tr("Schriftfarbe"), &colour_row, 2);

    // Date format — the system locale default (%x) follows the
    // system language; the other entries are explicit overrides
    // for users who want one fixed layout regardless of locale.
    let df = gtk4::DropDown::new(
        Some(gtk4::StringList::new(&[
            &tr("Systemsprache"),
            &tr("TT.MM.JJJJ"),
            &tr("JJJJ-MM-TT"),
            &tr("MM/TT/JJJJ"),
        ])),
        gtk4::Expression::NONE,
    );
    df.set_selected(match cfg::datefmt_saved() {
        cfg::DateFmt::System => 0,
        cfg::DateFmt::De => 1,
        cfg::DateFmt::Iso => 2,
        cfg::DateFmt::Us => 3,
    });
    df.connect_selected_notify(|d| {
        cfg::set_datefmt(match d.selected() {
            1 => cfg::DateFmt::De,
            2 => cfg::DateFmt::Iso,
            3 => cfg::DateFmt::Us,
            _ => cfg::DateFmt::System,
        });
    });
    attach(&grid, &tr("Datumsformat"), &df, 3);

    grid.upcast()
}

/// One labelled row — label left, widget right.
fn attach(grid: &gtk4::Grid, caption: &str, w: &impl IsA<gtk4::Widget>, row: i32) {
    let lbl = gtk4::Label::new(Some(caption));
    lbl.set_halign(gtk4::Align::Start);
    grid.attach(&lbl, 0, row, 1, 1);
    grid.attach(w, 1, row, 1, 1);
}
