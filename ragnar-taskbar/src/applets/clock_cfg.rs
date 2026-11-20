// Clock label configuration — every user-facing display option of
// the bar clock lives behind this module: weekday prefix, text
// colour, font weight and date format. All of them persist in
// clock.ini beneath the taskbar config dir and take effect live,
// so the settings dialog only calls the setters and the applet's
// per-second tick picks the new state up on its own.
use gtk4::glib;

/// Weekday display in the bar label: hidden, abbreviated (%a →
/// "Fr") or full name (%A → "Freitag"). Stored in clock.ini as a
/// string; the old boolean key migrates (true → abbreviated).
#[derive(Clone, Copy, PartialEq)]
pub enum Weekday {
    Off,
    Abbr,
    Full,
}

/// Date format for the tooltip and calendar panel header.
/// `System` uses the locale's %x (de → 20.11.2026, us → 11/20/2026);
/// the others are explicit overrides.
#[derive(Clone, Copy, PartialEq)]
pub enum DateFmt {
    System,
    De,
    Iso,
    Us,
}

/// Font weight of the bar label. `Normal` keeps plain set_text
/// rendering; anything else is applied through a Pango `weight`
/// span attribute.
#[derive(Clone, Copy, PartialEq)]
pub enum Weight {
    Normal,
    SemiBold,
    Bold,
}

impl Weight {
    /// Attribute value Pango understands in <span weight="…">.
    pub fn pango(self) -> &'static str {
        match self {
            Weight::Normal => "normal",
            Weight::SemiBold => "semibold",
            Weight::Bold => "bold",
        }
    }
}

thread_local! {
    /// Active display options — loaded once from clock.ini at
    /// applet start, updated live by the settings dialog.
    static WEEKDAY: std::cell::Cell<Weekday> = std::cell::Cell::new(weekday_saved());
    static COLOR: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(color_saved());
    static WEIGHT: std::cell::Cell<Weight> = std::cell::Cell::new(weight_saved());
    static DATEFMT: std::cell::Cell<DateFmt> = std::cell::Cell::new(datefmt_saved());
}

/// Current values for the per-tick repaint — one call instead of
/// four thread_local trips.
pub fn label_style() -> (Option<String>, Weight) {
    (
        COLOR.with(|c| c.borrow().clone()),
        WEIGHT.with(|c| c.get()),
    )
}

/// Weekday prefix fragment for strftime ("%a "/"%A " or empty).
pub fn weekday_prefix() -> &'static str {
    WEEKDAY.with(|c| match c.get() {
        Weekday::Off => "",
        Weekday::Abbr => "%a ",
        Weekday::Full => "%A ",
    })
}

/// Weekday name + date in the configured format — shared by the
/// bar tooltip, the calendar panel header and the settings live
/// preview so all three always agree.
pub fn date_label(now: &glib::DateTime) -> String {
    let pat = match DATEFMT.with(|c| c.get()) {
        DateFmt::System => "%A, %x",
        DateFmt::De => "%A, %d. %B %Y",
        DateFmt::Iso => "%A, %F",
        DateFmt::Us => "%A, %D",
    };
    now.format(pat).unwrap().to_string()
}

/// Load clock.ini (or a fresh keyfile) so a setter rewrites the
/// file with every existing key preserved — a blank KeyFile would
/// wipe the other options on every save.
fn ini() -> gtk4::glib::KeyFile {
    let kf = gtk4::glib::KeyFile::new();
    let _ = kf.load_from_file(
        crate::config::cfg_dir().join("clock.ini"),
        gtk4::glib::KeyFileFlags::NONE,
    );
    kf
}

/// Persist one key of the [clock] group and flush the file.
fn store(key: &str, value: Option<&str>) {
    let kf = ini();
    match value {
        Some(v) => kf.set_string("clock", key, v),
        None => {
            let _ = kf.remove_key("clock", key);
        }
    }
    let dir = crate::config::cfg_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = kf.save_to_file(dir.join("clock.ini"));
}

/// Read one string key of the [clock] group.
fn load(key: &str) -> Option<String> {
    let kf = gtk4::glib::KeyFile::new();
    let _ = kf.load_from_file(
        crate::config::cfg_dir().join("clock.ini"),
        gtk4::glib::KeyFileFlags::NONE,
    );
    kf.string("clock", key).ok().map(|s| s.to_string())
}

/// Persist + activate the weekday mode — called by the dialog.
pub fn set_weekday(mode: Weekday) {
    let s = match mode {
        Weekday::Off => "off",
        Weekday::Abbr => "abbr",
        Weekday::Full => "full",
    };
    store("weekday", Some(s));
    WEEKDAY.with(|c| c.set(mode));
}

pub fn weekday_saved() -> Weekday {
    if let Some(s) = load("weekday") {
        return match s.as_str() {
            "full" => Weekday::Full,
            "abbr" => Weekday::Abbr,
            _ => Weekday::Off,
        };
    }
    // Legacy boolean key: true meant the abbreviated name.
    let kf = gtk4::glib::KeyFile::new();
    let _ = kf.load_from_file(
        crate::config::cfg_dir().join("clock.ini"),
        gtk4::glib::KeyFileFlags::NONE,
    );
    match kf.boolean("clock", "weekday") {
        Ok(true) => Weekday::Abbr,
        _ => Weekday::Off,
    }
}

/// Persist + activate the label colour — None keeps the theme colour.
pub fn set_color(hex: Option<String>) {
    store("color", hex.as_deref());
    COLOR.with(|c| *c.borrow_mut() = hex);
}

pub fn color_saved() -> Option<String> {
    match load("color") {
        Some(s) if s.starts_with('#') => Some(s),
        _ => None,
    }
}

/// Persist + activate the font weight — Normal skips markup
/// entirely so the label renders exactly like the theme default.
pub fn set_weight(w: Weight) {
    store("weight", Some(w.pango()));
    WEIGHT.with(|c| c.set(w));
}

pub fn weight_saved() -> Weight {
    match load("weight").as_deref() {
        Some("semibold") => Weight::SemiBold,
        Some("bold") => Weight::Bold,
        _ => Weight::Normal,
    }
}

/// Persist + activate the date format — called by the dialog.
pub fn set_datefmt(mode: DateFmt) {
    let s = match mode {
        DateFmt::System => "system",
        DateFmt::De => "de",
        DateFmt::Iso => "iso",
        DateFmt::Us => "us",
    };
    store("datefmt", Some(s));
    DATEFMT.with(|c| c.set(mode));
}

pub fn datefmt_saved() -> DateFmt {
    match load("datefmt").as_deref() {
        Some("de") => DateFmt::De,
        Some("iso") => DateFmt::Iso,
        Some("us") => DateFmt::Us,
        _ => DateFmt::System,
    }
}
