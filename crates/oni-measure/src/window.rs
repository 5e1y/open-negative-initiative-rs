//! Where a measurement reads, in Core Image's frame: x right, y up, an extent's lower-left corner
//! its origin. `Pipeline.measurementWindows`, `Geometry`'s measuring window and a sheet's cells.

use oni_settings::cg::{Point, Rect};
use oni_settings::frame::{ContactSheetGrid, Geometry};
use oni_settings::PipelineSettings;

/// `Geometry.measureWindowInset`: how far inside the crop a proposed window starts, on each side.
pub const MEASURE_WINDOW_INSET: f64 = 0.1;

pub const FULL_WINDOW: Rect = Rect::UNIT;

pub fn max_x(r: &Rect) -> f64 {
    r.x + r.width
}

pub fn max_y(r: &Rect) -> f64 {
    r.y + r.height
}

pub fn is_empty(r: &Rect) -> bool {
    !(r.width > 0.0 && r.height > 0.0)
}

/// `CGRect.intersection`; disjoint rectangles give an empty one.
pub fn intersection(a: &Rect, b: &Rect) -> Rect {
    let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
    let (x1, y1) = (max_x(a).min(max_x(b)), max_y(a).min(max_y(b)));
    if x1 < x0 || y1 < y0 {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

pub fn union(a: &Rect, b: &Rect) -> Rect {
    let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
    Rect::new(x0, y0, max_x(a).max(max_x(b)) - x0, max_y(a).max(max_y(b)) - y0)
}

/// `CGRect.integral`: the smallest rectangle of whole pixels holding `r`.
pub fn integral(r: &Rect) -> Rect {
    let (x0, y0) = (r.x.floor(), r.y.floor());
    Rect::new(x0, y0, max_x(r).ceil() - x0, max_y(r).ceil() - y0)
}

/// `ContactSheet.Grid.whole`: rounded inward, so no partly covered pixel on a border is counted.
pub fn whole(r: &Rect) -> Rect {
    let (x, y) = (r.x.ceil(), r.y.ceil());
    Rect::new(x, y, max_x(r).floor() - x, max_y(r).floor() - y)
}

pub fn inset(r: &Rect, dx: f64, dy: f64) -> Rect {
    Rect::new(r.x + dx, r.y + dy, r.width - 2.0 * dx, r.height - 2.0 * dy)
}

/// `CGRect.contains(CGRect)`, edges included.
pub fn contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x && inner.y >= outer.y && max_x(inner) <= max_x(outer) && max_y(inner) <= max_y(outer)
}

/// `CGRect.contains(CGPoint)`: the low edges in, the high edges out.
pub fn contains_point(r: &Rect, p: Point) -> bool {
    p.x >= r.x && p.x < max_x(r) && p.y >= r.y && p.y < max_y(r)
}

/// `Geometry.relative`: `rect` seen from inside `base`; a rect missing `base` reads as all of it.
pub fn relative(rect: &Rect, base: &Rect) -> Rect {
    let cut = intersection(rect, base);
    if !(base.width > 0.0 && base.height > 0.0 && cut.width > 0.0 && cut.height > 0.0) {
        return FULL_WINDOW;
    }
    Rect::new((cut.x - base.x) / base.width, (cut.y - base.y) / base.height, cut.width / base.width, cut.height / base.height)
}

/// `Geometry.absolute`: a rect drawn inside `base`, back in the frame's fractions.
pub fn absolute(rect: &Rect, base: &Rect) -> Rect {
    Rect::new(base.x + rect.x * base.width, base.y + rect.y * base.height, rect.width * base.width, rect.height * base.height)
}

/// `Geometry.measureWindowInCrop`: the window over the picture stage 0 hands on; `None` restricts nothing.
pub fn in_crop(g: &Geometry) -> Option<Rect> {
    let window = g.measure_window?;
    let seen = relative(&window, if g.crop_bypassed { &FULL_WINDOW } else { &g.crop });
    (seen != FULL_WINDOW).then_some(seen)
}

/// `Geometry.proposal`: inside the crop and clear of its edges, a sheet's on the whole crop.
pub fn proposal(crop: &Rect, sheet: bool) -> Rect {
    let inset_by = if sheet { 0.0 } else { MEASURE_WINDOW_INSET };
    inset(crop, crop.width * inset_by, crop.height * inset_by)
}

/// `Geometry.proposeMeasureWindow`: an untouched window gets one; a placed one stays.
pub fn propose(g: &mut Geometry, sheet: bool) {
    if g.measure_window.is_none() {
        g.measure_window = Some(proposal(&g.crop, sheet));
    }
}

/// `Pipeline.measurementWindows`: the drawn window, else the picture clear of its edges; empty is the whole.
pub fn measurement_windows(extent: &Rect, s: &PipelineSettings) -> Vec<Rect> {
    let i = MEASURE_WINDOW_INSET;
    let untouched = s.geometry.measure_window.is_none();
    let Some(seen) = in_crop(&s.geometry).or(untouched.then(|| Rect::new(i, i, 1.0 - 2.0 * i, 1.0 - 2.0 * i))) else {
        return Vec::new();
    };
    let drawn = whole(&Rect::new(
        extent.x + seen.x * extent.width,
        extent.y + seen.y * extent.height,
        seen.width * extent.width,
        seen.height * extent.height,
    ));
    if drawn.width > 0.0 && drawn.height > 0.0 { vec![drawn] } else { Vec::new() }
}

/// `ContactSheet.Grid.reads`: each frame's own window where the sheet carries one per frame, else its cell.
pub fn sheet_reads(grid: &ContactSheetGrid, extent: &Rect) -> Vec<Rect> {
    let Some(windows) = grid.windows.as_ref().filter(|w| w.len() as i64 == grid.frames) else {
        return sheet_cells(grid, extent);
    };
    windows
        .iter()
        .filter_map(|w| {
            let at = Rect::new(extent.x + w.x * extent.width, extent.y + w.y * extent.height, w.width * extent.width, w.height * extent.height);
            let read = whole(&intersection(&at, extent));
            (read.width >= 1.0 && read.height >= 1.0).then_some(read)
        })
        .collect()
}

/// `ContactSheet.Grid.cells`: the tiles as laid, the first row at the top. A sidecar's grid without
/// rows or columns lays nothing, and frames past its last cell fall below the extent.
pub fn sheet_cells(grid: &ContactSheetGrid, extent: &Rect) -> Vec<Rect> {
    if grid.columns <= 0 || grid.rows <= 0 {
        return Vec::new();
    }
    let columns = grid.columns as f64;
    let laid = columns * grid.tile.width + (columns + 1.0) * grid.margin;
    let scale = if laid > 0.0 { extent.width / laid } else { 1.0 };
    let (w, h, m) = (grid.tile.width * scale, grid.tile.height * scale, grid.margin * scale);
    (0..grid.frames.clamp(0, grid.columns.saturating_mul(grid.rows)))
        .filter_map(|index| {
            let row = (grid.rows - 1 - index / grid.columns) as f64;
            let at = Rect::new(extent.x + m + (index % grid.columns) as f64 * (w + m), extent.y + m + row * (h + m), w, h);
            let cell = whole(&intersection(&at, extent));
            (cell.width >= 1.0 && cell.height >= 1.0).then_some(cell)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg::Size;

    fn extent() -> Rect {
        Rect::new(0.0, 0.0, 1400.0, 933.0)
    }

    #[test]
    fn an_untouched_frame_reads_clear_of_its_edges_and_a_drawn_window_where_it_was_drawn() {
        let mut s = PipelineSettings::default();
        let rest = measurement_windows(&extent(), &s);
        // 140 to 1260 across, 93.3 to 839.7 up, rounded inward.
        assert_eq!(rest, vec![Rect::new(140.0, 94.0, 1120.0, 745.0)]);
        s.geometry.measure_window = Some(Rect::new(0.5, 0.0, 0.5, 1.0));
        assert_eq!(measurement_windows(&extent(), &s), vec![Rect::new(700.0, 0.0, 700.0, 933.0)]);
        // Twin: a window over the whole crop restricts nothing, so the whole picture reads.
        s.geometry.measure_window = Some(Rect::UNIT);
        assert!(measurement_windows(&extent(), &s).is_empty());
        assert_ne!(measurement_windows(&extent(), &s), rest);
    }

    #[test]
    fn the_window_is_seen_inside_the_crop_and_a_bypassed_crop_reads_the_frame() {
        let mut g = Geometry { crop: Rect::new(0.2, 0.2, 0.6, 0.6), ..Geometry::default() };
        g.measure_window = Some(Rect::new(0.2, 0.2, 0.3, 0.6));
        let seen = in_crop(&g).unwrap();
        assert!((seen.x).abs() < 1e-12 && (seen.width - 0.5).abs() < 1e-12 && (seen.height - 1.0).abs() < 1e-12);
        g.crop_bypassed = true;
        // Twin: off the crop the same window is read in the frame's own fractions.
        let whole = in_crop(&g).unwrap();
        assert!((whole.x - 0.2).abs() < 1e-12 && (whole.width - 0.3).abs() < 1e-12 && (whole.height - 0.6).abs() < 1e-12);
        // A window missing the crop altogether reads the whole of it.
        assert_eq!(relative(&Rect::new(0.9, 0.9, 0.05, 0.05), &Rect::new(0.0, 0.0, 0.5, 0.5)), FULL_WINDOW);
    }

    #[test]
    fn a_proposal_insets_a_frame_by_a_tenth_and_a_sheet_not_at_all() {
        let crop = Rect::new(0.1, 0.0, 0.8, 1.0);
        let p = proposal(&crop, false);
        assert!((p.x - 0.18).abs() < 1e-12 && (p.width - 0.64).abs() < 1e-12 && (p.y - 0.1).abs() < 1e-12);
        assert_eq!(proposal(&crop, true), crop);
        let mut g = Geometry { measure_window: Some(Rect::new(0.3, 0.3, 0.1, 0.1)), ..Geometry::default() };
        propose(&mut g, false);
        // Twin: a window the hand placed is left where it is.
        assert_eq!(g.measure_window, Some(Rect::new(0.3, 0.3, 0.1, 0.1)));
    }

    #[test]
    fn a_sheet_grid_without_columns_or_rows_lays_no_cell() {
        let grid = |columns, rows, frames| ContactSheetGrid { columns, rows, frames, tile: Size { width: 36.0, height: 24.0 }, margin: 2.0, windows: None };
        assert!(sheet_reads(&grid(0, 2, 4), &extent()).is_empty());
        assert!(sheet_cells(&grid(3, 0, 4), &extent()).is_empty());
        assert!(sheet_cells(&grid(-1, 2, 4), &extent()).is_empty());
        // Twin: a whole grid still lays its six, and frames past them add none, however many.
        assert_eq!(sheet_cells(&grid(3, 2, 6), &extent()).len(), 6);
        assert_eq!(sheet_cells(&grid(3, 2, 1000), &extent()), sheet_cells(&grid(3, 2, 6), &extent()));
        assert_eq!(sheet_cells(&grid(3, 2, i64::MAX), &extent()).len(), 6);
    }

    #[test]
    fn whole_rounds_inward_and_integral_outward() {
        let r = Rect::new(1.5, 2.25, 10.0, 4.5);
        assert_eq!(whole(&r), Rect::new(2.0, 3.0, 9.0, 3.0));
        assert_eq!(integral(&r), Rect::new(1.0, 2.0, 11.0, 5.0));
    }
}
