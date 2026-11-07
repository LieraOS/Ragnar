//! Icon layout: grid geometry, placement of icons on the desktop
//! and persistence of their positions.
//!
//! Positions are stored per item id in `~/.config/ragnar-view/layout`
//! as a serialized GVariant `a{s(ii)}` (same idea as the old view's
//! layout file). All placement logic is pure so it can be tested.

use gtk4::glib::{self, prelude::*};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// Grid cell size and the margin around the grid.
pub const CELL_W: i32 = 104;
pub const CELL_H: i32 = 110;
pub const MARGIN: i32 = 16;
/// Size of one icon widget (must fit inside a cell).
pub const ICON_W: i32 = 96;
pub const ICON_H: i32 = 102;

/// Item id → top-left pixel position.
pub type Positions = HashMap<String, (i32, i32)>;

/// How icons are arranged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrange {
    /// Free dragging, released icons snap into grid cells.
    Grid,
    /// Free dragging, pixel-exact placement.
    Free,
    /// Icons flow in sorted order; dragging is disabled.
    Auto,
}

impl Arrange {
    /// Parse the GSettings value (unknown → Grid).
    pub fn from_key(k: &str) -> Self {
        match k {
            "free" => Self::Free,
            "auto" => Self::Auto,
            _ => Self::Grid,
        }
    }
}

/// Usable desktop size in pixels (the main monitor).
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub w: i32,
    pub h: i32,
}

impl Geo {
    /// Number of grid columns / rows that fit.
    pub fn cols(&self) -> i32 {
        ((self.w - 2 * MARGIN) / CELL_W).max(1)
    }
    pub fn rows(&self) -> i32 {
        ((self.h - 2 * MARGIN) / CELL_H).max(1)
    }

    /// Pixel position of grid cell `(col, row)`.
    pub fn cell_xy(&self, c: (i32, i32)) -> (i32, i32) {
        (MARGIN + c.0 * CELL_W, MARGIN + c.1 * CELL_H)
    }

    /// Grid cell nearest to a pixel position.
    pub fn snap(&self, p: (i32, i32)) -> (i32, i32) {
        let col = ((p.0 - MARGIN) as f64 / CELL_W as f64).round() as i32;
        let row = ((p.1 - MARGIN) as f64 / CELL_H as f64).round() as i32;
        (col.clamp(0, self.cols() - 1), row.clamp(0, self.rows() - 1))
    }

    /// Keep an icon fully on screen.
    pub fn clamp(&self, p: (i32, i32)) -> (i32, i32) {
        (
            p.0.clamp(0, (self.w - ICON_W).max(0)),
            p.1.clamp(0, (self.h - ICON_H).max(0)),
        )
    }

    /// Free cell closest to `want`, or None when the grid is full.
    pub fn nearest_free(
        &self,
        want: (i32, i32),
        taken: &HashSet<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        let dist = |c: &(i32, i32)| (c.0 - want.0).pow(2) + (c.1 - want.1).pow(2);
        (0..self.cols())
            .flat_map(|c| (0..self.rows()).map(move |r| (c, r)))
            .filter(|c| !taken.contains(c))
            .min_by_key(dist)
    }

    /// First free cell filling columns top to bottom.
    pub fn first_free(&self, taken: &HashSet<(i32, i32)>) -> Option<(i32, i32)> {
        (0..self.cols())
            .flat_map(|c| (0..self.rows()).map(move |r| (c, r)))
            .find(|c| !taken.contains(c))
    }
}

/// Decide where every item goes. `saved` is updated with the final
/// positions (except in Auto mode, where nothing is remembered).
pub fn place(ids: &[String], saved: &mut Positions, geo: Geo, mode: Arrange) -> Vec<(i32, i32)> {
    let mut taken: HashSet<(i32, i32)> = HashSet::new();
    if mode == Arrange::Auto {
        return (0..ids.len() as i32)
            .map(|i| geo.cell_xy((i / geo.rows(), i % geo.rows())))
            .collect();
    }
    let mut out: Vec<Option<(i32, i32)>> = vec![None; ids.len()];
    // Pass 1: items that already have a remembered position.
    for (i, id) in ids.iter().enumerate() {
        let Some(&p) = saved.get(id) else { continue };
        let p = if mode == Arrange::Grid {
            let cell = geo.snap(p);
            // Two icons snapped onto one cell: the later one moves.
            if taken.contains(&cell) {
                continue;
            }
            geo.cell_xy(cell)
        } else {
            geo.clamp(p)
        };
        taken.insert(geo.snap(p));
        out[i] = Some(p);
    }
    // Pass 2: new (or displaced) items take the first free cells.
    for slot in out.iter_mut().filter(|s| s.is_none()) {
        let cell = geo
            .first_free(&taken)
            .unwrap_or((geo.cols() - 1, geo.rows() - 1));
        taken.insert(cell);
        *slot = Some(geo.cell_xy(cell));
    }
    let placed: Vec<(i32, i32)> = out.into_iter().flatten().collect();
    for (id, p) in ids.iter().zip(&placed) {
        saved.insert(id.clone(), *p);
    }
    placed
}

/// Final position of a dragged icon. `others` are the positions of
/// all other icons (used to avoid stacking in Grid mode).
pub fn resolve_drop(geo: Geo, mode: Arrange, raw: (i32, i32), others: &[(i32, i32)]) -> (i32, i32) {
    match mode {
        Arrange::Grid => {
            let taken: HashSet<_> = others.iter().map(|&p| geo.snap(p)).collect();
            let want = geo.snap(raw);
            geo.cell_xy(geo.nearest_free(want, &taken).unwrap_or(want))
        }
        _ => geo.clamp(raw),
    }
}

/// Location of the layout file for one monitor connector. Every
/// output keeps its own placement: icon positions only make sense
/// within that monitor's geometry.
fn file(conn: &str) -> PathBuf {
    glib::user_config_dir()
        .join("ragnar-view")
        .join(format!("layout-{conn}"))
}

/// Single-monitor layout file written by older versions; kept as a
/// read fallback so existing placements survive the switch.
fn legacy_file() -> PathBuf {
    glib::user_config_dir().join("ragnar-view").join("layout")
}

/// Parse a positions file (empty when missing or damaged).
fn parse(path: &PathBuf) -> Positions {
    let Ok(bytes) = std::fs::read(path) else {
        return Positions::new();
    };
    let v = glib::Variant::from_data::<Positions, _>(bytes);
    if !v.is_normal_form() {
        return Positions::new();
    }
    v.get::<Positions>().unwrap_or_default()
}

/// Every connector's remembered positions plus the legacy file
/// under key "" — the caller maps that onto the primary output.
/// An item id lives in at most one connector's section: that is
/// what decides which monitor shows it.
pub fn load_all() -> BTreeMap<String, Positions> {
    let mut all = BTreeMap::new();
    let dir = glib::user_config_dir().join("ragnar-view");
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(conn) = name.strip_prefix("layout-") else {
                continue;
            };
            all.insert(conn.to_string(), parse(&e.path()));
        }
    }
    let legacy = parse(&legacy_file());
    if !legacy.is_empty() {
        all.insert(String::new(), legacy);
    }
    all
}

/// Move `id` into `conn`'s section at `pos`, removing it from any
/// other monitor. Every touched file is saved — the one-monitor-
/// one-home rule is what keeps an icon from showing up twice.
pub fn assign(all: &mut BTreeMap<String, Positions>, conn: &str, id: &str, pos: (i32, i32)) {
    for (c, p) in all.iter_mut() {
        if c != conn && p.remove(id).is_some() {
            save(c, p);
        }
    }
    let mine = all.entry(conn.to_string()).or_default();
    mine.insert(id.to_string(), pos);
    let mine = all.get(conn).expect("just inserted");
    save(conn, mine);
}

/// Persist positions for `conn` atomically (temp file + rename).
pub fn save(conn: &str, p: &Positions) {
    let path = file(conn);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    let data = p.to_variant().data_as_bytes();
    if std::fs::write(&tmp, &data).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Geo = Geo { w: 1920, h: 1200 };

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("i{i}")).collect()
    }

    #[test]
    fn snap_rounds_to_nearest_cell_and_clamps() {
        assert_eq!(G.snap((MARGIN + 3, MARGIN + 2)), (0, 0));
        assert_eq!(G.snap((MARGIN + CELL_W + 40, MARGIN + CELL_H - 30)), (1, 1));
        assert_eq!(G.snap((-500, -500)), (0, 0));
        assert_eq!(G.snap((99999, 99999)), (G.cols() - 1, G.rows() - 1));
    }

    #[test]
    fn new_items_fill_columns_top_to_bottom() {
        let mut saved = Positions::new();
        let p = place(&ids(3), &mut saved, G, Arrange::Grid);
        assert_eq!(p[0], G.cell_xy((0, 0)));
        assert_eq!(p[1], G.cell_xy((0, 1)));
        assert_eq!(p[2], G.cell_xy((0, 2)));
        assert_eq!(saved.len(), 3);
    }

    #[test]
    fn saved_position_is_kept_and_new_items_avoid_it() {
        let mut saved = Positions::new();
        saved.insert("i0".into(), G.cell_xy((0, 0)));
        let p = place(&ids(2), &mut saved, G, Arrange::Grid);
        assert_eq!(p[0], G.cell_xy((0, 0)));
        assert_eq!(p[1], G.cell_xy((0, 1)));
    }

    #[test]
    fn two_icons_on_one_cell_get_separated_in_grid_mode() {
        let mut saved = Positions::new();
        saved.insert("i0".into(), G.cell_xy((2, 2)));
        saved.insert("i1".into(), G.cell_xy((2, 2)));
        let p = place(&ids(2), &mut saved, G, Arrange::Grid);
        assert_ne!(p[0], p[1]);
    }

    #[test]
    fn free_mode_keeps_pixel_positions() {
        let mut saved = Positions::new();
        saved.insert("i0".into(), (333, 217));
        let p = place(&ids(1), &mut saved, G, Arrange::Free);
        assert_eq!(p[0], (333, 217));
    }

    #[test]
    fn auto_mode_ignores_and_keeps_no_positions() {
        let mut saved = Positions::new();
        saved.insert("i0".into(), (333, 217));
        let p = place(&ids(2), &mut saved, G, Arrange::Auto);
        assert_eq!(p[0], G.cell_xy((0, 0)));
        assert_eq!(saved.get("i0"), Some(&(333, 217)));
        assert_eq!(saved.len(), 1);
    }

    #[test]
    fn drop_snaps_and_dodges_occupied_cells() {
        let others = [G.cell_xy((3, 3))];
        let raw = G.cell_xy((3, 3));
        let spot = resolve_drop(G, Arrange::Grid, raw, &others);
        assert_ne!(spot, G.cell_xy((3, 3)));
        let free = resolve_drop(G, Arrange::Free, (10, 10), &others);
        assert_eq!(free, (10, 10));
    }

    #[test]
    fn positions_survive_a_variant_round_trip() {
        let mut p = Positions::new();
        p.insert("file:///a b".into(), (120, 340));
        let v = glib::Variant::from_data::<Positions, _>(p.to_variant().data_as_bytes());
        assert!(v.is_normal_form());
        assert_eq!(v.get::<Positions>().unwrap(), p);
    }
}
