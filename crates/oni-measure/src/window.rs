//! Where a measurement reads, in Core Image's frame: x right, y up, an extent's lower-left corner
//! its origin. `Pipeline.measurementWindows` and a sheet's cells; the window's rule is `oni_settings::geometry`'s.

use oni_settings::cg::{Point, Rect};
use oni_settings::frame::ContactSheetGrid;
use oni_settings::geometry::proposal;
use oni_settings::PipelineSettings;

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

/// `Pipeline.measurementWindows`: the drawn window, else the picture clear of its edges; empty is the whole.
pub fn measurement_windows(extent: &Rect, s: &PipelineSettings) -> Vec<Rect> {
    let untouched = s.geometry.measure_window.is_none();
    let Some(seen) = s.geometry.measure_window_in_crop().or(untouched.then(|| proposal(Rect::UNIT, false))) else {
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
