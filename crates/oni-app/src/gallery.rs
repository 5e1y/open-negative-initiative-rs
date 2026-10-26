//! `GalleryLayout`: where every cell is drawn, computed and never measured. The count is fixed and
//! the tile follows the width; a scroll asks only for the range of cells in view.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub margin: f32,
    pub gutter: f32,
    pub padding: f32,
    /// Zero where a surface draws no header.
    pub header: f32,
    pub caption: f32,
    pub caption_gap: f32,
}

impl Metrics {
    /// `GalleryEngine.metrics`: no caption reserved, as the sidebar names every photo and text
    /// layout costs at hundreds of cells.
    pub const GALLERY: Metrics = Metrics { margin: 24.0, gutter: 8.0, padding: 24.0, header: 37.0, caption: 0.0, caption_gap: 0.0 };
}

/// `DS.Control.columnSteps`, dense first: zooming in steps right, to fewer and larger tiles.
pub const COLUMN_STEPS: [usize; 2] = [12, 6];

/// `GalleryView.columnStep`: one rung away, clamped, a count off a rung first snapping to the nearest.
pub fn column_step(count: usize, by: i32) -> usize {
    let here = (0..COLUMN_STEPS.len()).min_by_key(|&i| COLUMN_STEPS[i].abs_diff(count)).unwrap_or(0);
    COLUMN_STEPS[(here as i32 + by).clamp(0, COLUMN_STEPS.len() as i32 - 1) as usize]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub title: String,
    pub caption: String,
    pub start: usize,
    pub count: usize,
    pub top: f32,
    pub grid_top: f32,
    pub row_count: usize,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub metrics: Metrics,
    pub columns: usize,
    pub width: f32,
    /// The picture; the cell adds the caption under it.
    pub tile: (f32, f32),
    pub cell: (f32, f32),
    pub bands: Vec<Band>,
    pub count: usize,
    pub height: f32,
}

/// Rows of slack laid on both sides of the viewport, so a scroll never shows a cell being made.
pub const OVERSCAN: usize = 1;

impl Layout {
    /// `groups` are each band's title, caption and count, in document order.
    pub fn new(groups: &[(String, String, usize)], columns: usize, width: f32, metrics: Metrics) -> Layout {
        let n = columns.max(1);
        let usable = (width - 2.0 * metrics.margin - (n - 1) as f32 * metrics.gutter).max(0.0);
        let tile_width = usable / n as f32;
        let tile_height = tile_width * 2.0 / 3.0;
        let cell_height = tile_height + metrics.caption_gap + metrics.caption;
        let mut bands = Vec::new();
        let mut y = metrics.padding;
        let mut start = 0;
        for (title, caption, count) in groups.iter().filter(|g| g.2 > 0) {
            let top = y;
            if metrics.header > 0.0 {
                y += metrics.header + metrics.gutter;
            }
            let rows = count.div_ceil(n);
            let grid_top = y;
            y += rows as f32 * cell_height + (rows - 1) as f32 * metrics.gutter;
            bands.push(Band {
                title: title.clone(),
                caption: caption.clone(),
                start,
                count: *count,
                top,
                grid_top,
                row_count: rows,
                bottom: y,
            });
            start += count;
            y += metrics.gutter;
        }
        if !bands.is_empty() {
            y -= metrics.gutter;
        }
        let height = if bands.is_empty() { 0.0 } else { y + metrics.padding };
        Layout { metrics, columns: n, width, tile: (tile_width, tile_height), cell: (tile_width, cell_height), bands, count: start, height }
    }

    pub fn row_pitch(&self) -> f32 {
        self.cell.1 + self.metrics.gutter
    }

    fn column_pitch(&self) -> f32 {
        self.cell.0 + self.metrics.gutter
    }

    pub fn band(&self, index: usize) -> Option<&Band> {
        if index >= self.count {
            return None;
        }
        let at = self.bands.partition_point(|b| b.start <= index);
        self.bands.get(at.checked_sub(1)?)
    }

    pub fn frame(&self, index: usize) -> Option<Rect> {
        let band = self.band(index)?;
        let local = index - band.start;
        Some(Rect {
            x: self.metrics.margin + (local % self.columns) as f32 * (self.tile.0 + self.metrics.gutter),
            y: band.grid_top + (local / self.columns) as f32 * self.row_pitch(),
            width: self.cell.0,
            height: self.cell.1,
        })
    }

    fn row(&self, y: f32, band: &Band) -> usize {
        let raw = (y - band.grid_top) / self.row_pitch();
        if raw <= 0.0 { 0 } else { (band.row_count - 1).min(raw as usize) }
    }

    /// The cell under a point, clamped to the nearest band, column or row rather than refused.
    pub fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let at = self.bands.iter().position(|b| y < b.bottom).unwrap_or(self.bands.len() - 1);
        let band = &self.bands[at];
        let column = (((x - self.metrics.margin) / self.column_pitch()).max(0.0) as usize).min(self.columns - 1);
        let local = self.row(y, band) * self.columns + column;
        Some((band.start + band.count - 1).min(band.start + local))
    }

    /// "Before this index", `0..=count`, so the far side of the last cell is a position too.
    pub fn boundary(&self, x: f32, y: f32) -> Option<usize> {
        let near = self.index_at(x, y)?;
        let frame = self.frame(near)?;
        Some(if x < frame.x + frame.width / 2.0 { near } else { near + 1 })
    }

    /// The drop line a boundary draws: in the gutter before its cell, or after the last one.
    pub fn boundary_line(&self, boundary: usize, thickness: f32) -> Option<Rect> {
        if self.count == 0 {
            return None;
        }
        let clamped = boundary.min(self.count);
        let r = self.frame(clamped.min(self.count - 1))?;
        let half = self.metrics.gutter / 2.0;
        if self.columns == 1 {
            let y = if clamped == self.count { r.y + r.height + half } else { r.y - half };
            return Some(Rect { x: r.x, y: y - thickness / 2.0, width: r.width, height: thickness });
        }
        let x = if clamped == self.count { r.x + r.width + half } else { r.x - half };
        Some(Rect { x: x - thickness / 2.0, y: r.y, width: thickness, height: r.height })
    }

    /// The cells between `top` and `top + viewport`, one row of slack either side: one range, since
    /// index order follows the document's.
    pub fn cells(&self, top: f32, viewport: f32) -> Range<usize> {
        if self.count == 0 || viewport <= 0.0 {
            return 0..0;
        }
        let slack = OVERSCAN as f32 * self.row_pitch();
        let (low, high) = (top - slack, top + viewport + slack);
        let Some(head) = self.bands.iter().position(|b| b.bottom > low) else { return 0..0 };
        let Some(tail) = self.bands.iter().rposition(|b| b.top < high) else { return 0..0 };
        if tail < head {
            return 0..0;
        }
        let (head, tail) = (&self.bands[head], &self.bands[tail]);
        let first = head.start + self.row(low, head) * self.columns;
        let last = (tail.start + tail.count - 1).min(tail.start + self.row(high, tail) * self.columns + self.columns - 1);
        if first > last { 0..0 } else { first..last + 1 }
    }

    /// The most cells `cells` gives for a viewport, wherever it scrolls: the tiles keep one slot each
    /// while the grid moves, so a scroll remakes no tile.
    pub fn slots(&self, viewport: f32) -> usize {
        if self.count == 0 || viewport <= 0.0 {
            return 0;
        }
        let span = viewport + 2.0 * OVERSCAN as f32 * self.row_pitch();
        // A row cut at each end of the span, one more where a band ends inside it.
        let rows = (span / self.row_pitch()).ceil() as usize + 2;
        (rows * self.columns).min(self.count)
    }

    /// The bands whose header is in view, by index.
    pub fn headers(&self, top: f32, viewport: f32) -> Range<usize> {
        if self.bands.is_empty() || viewport <= 0.0 {
            return 0..0;
        }
        let slack = OVERSCAN as f32 * self.row_pitch();
        let (low, high) = (top - slack, top + viewport + slack);
        let first = self.bands.iter().position(|b| b.bottom > low);
        let last = self.bands.iter().rposition(|b| b.top < high);
        match (first, last) {
            (Some(f), Some(l)) if l >= f => f..l + 1,
            _ => 0..0,
        }
    }

    /// `reveal(_:from:in:)`: the shortest move that shows a cell, clearing the pinned header too;
    /// `None` when it is already in view.
    pub fn reveal(&self, index: usize, top: f32, viewport: f32) -> Option<f32> {
        let frame = self.frame(index)?;
        let (gutter, header) = (self.metrics.gutter, self.metrics.header);
        let wanted = if frame.y - gutter - header < top {
            let band_top = self.band(index).map_or(0.0, |b| b.top);
            (frame.y - gutter - header).max(band_top)
        } else if frame.y + frame.height + gutter > top + viewport {
            frame.y + frame.height + gutter - viewport
        } else {
            return None;
        };
        let clamped = wanted.max(0.0).min((self.height - viewport).max(0.0));
        ((clamped - top).abs() >= 0.5).then_some(clamped)
    }

    /// `offset(landing:in:)`: the cell's middle at the viewport's, clamped to the content; the top
    /// when no cell is named.
    pub fn landing(&self, index: Option<usize>, viewport: f32) -> f32 {
        let Some(frame) = index.and_then(|i| self.frame(i)) else { return 0.0 };
        (frame.y + frame.height / 2.0 - viewport / 2.0).max(0.0).min((self.height - viewport).max(0.0))
    }

    /// `header(at:)` with `pin(of:at:)`: the band under the top edge and where its header is held
    /// until the band's end pushes it off; `None` while the band's own header shows in place.
    pub fn pinned(&self, top: f32) -> Option<(usize, f32)> {
        if self.metrics.header <= 0.0 {
            return None;
        }
        let index = self.bands.iter().rposition(|b| b.top <= top).unwrap_or(0);
        let band = self.bands.get(index)?;
        let y = band.top.max(top).min(band.bottom - self.metrics.header);
        (y > band.top).then_some((index, y))
    }

    /// The scroll offset kept inside the content.
    pub fn clamped(&self, scroll: f32, viewport: f32) -> f32 {
        scroll.clamp(0.0, (self.height - viewport).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_moves_the_least_to_show_a_cell_and_not_at_all_when_it_shows() {
        let l = Layout::new(&roll(60), 6, 1000.0, Metrics::GALLERY);
        let viewport = 400.0;
        assert_eq!(l.reveal(0, 0.0, viewport), None, "the first cell is in view at the top");
        let last = l.frame(59).unwrap();
        let down = l.reveal(59, 0.0, viewport).unwrap();
        assert!((down - (last.y + last.height + l.metrics.gutter - viewport)).abs() < 1e-3, "{down}");
        // Twin: from the bottom, the first cell scrolls back to its band's top, never above zero.
        let up = l.reveal(0, down, viewport).unwrap();
        assert!(up >= 0.0 && up <= l.frame(0).unwrap().y, "{up}");
        assert_eq!(l.reveal(59, down, viewport), None);
    }

    #[test]
    fn a_landing_centres_its_cell_within_the_content_and_tops_without_one() {
        let l = Layout::new(&roll(120), 6, 1000.0, Metrics::GALLERY);
        let viewport = 400.0;
        let middle = l.frame(60).unwrap();
        let landed = l.landing(Some(60), viewport);
        assert!((landed + viewport / 2.0 - (middle.y + middle.height / 2.0)).abs() < 1e-3, "{landed}");
        assert_eq!(l.landing(Some(119), viewport), l.height - viewport, "the last row clamps to the end");
        assert_eq!(l.landing(Some(0), viewport), 0.0, "the first row clamps to the top");
        assert_eq!(l.landing(None, viewport), 0.0);
        // Twin: the reveal's shortest move leaves the middle cell at the viewport's edge, not centred.
        let revealed = l.reveal(60, 0.0, viewport).unwrap();
        assert!((revealed - landed).abs() > 1.0, "{revealed} {landed}");
    }

    #[test]
    fn a_drop_lands_before_the_nearer_half_and_past_the_last_cell() {
        let l = Layout::new(&roll(10), 6, 1000.0, Metrics::GALLERY);
        let c = l.frame(3).unwrap();
        assert_eq!(l.boundary(c.x + 1.0, c.y + 1.0), Some(3));
        assert_eq!(l.boundary(c.x + c.width - 1.0, c.y + 1.0), Some(4));
        let last = l.frame(9).unwrap();
        assert_eq!(l.boundary(last.x + last.width - 1.0, last.y + 1.0), Some(10));
        let line = l.boundary_line(10, 2.0).unwrap();
        assert_eq!(line.x, last.x + last.width + 4.0 - 1.0);
        // Twin: the line before a cell sits in the gutter on its left, not its right.
        assert_eq!(l.boundary_line(3, 2.0).unwrap().x, c.x - 4.0 - 1.0);
    }

    #[test]
    fn a_band_s_header_is_held_at_the_top_until_its_end_pushes_it_off() {
        let groups = vec![("A".to_string(), String::new(), 30), ("B".to_string(), String::new(), 30)];
        let l = Layout::new(&groups, 6, 1000.0, Metrics::GALLERY);
        let (a, b) = (&l.bands[0], &l.bands[1]);
        assert_eq!(l.pinned(0.0), None, "the first header shows in place at the top");
        assert_eq!(l.pinned(a.top + 100.0), Some((0, a.top + 100.0)), "held at the top edge");
        let pushed = a.bottom - 10.0;
        assert_eq!(l.pinned(pushed), Some((0, a.bottom - Metrics::GALLERY.header)), "the band's end pushes it up");
        // Twin: past the second band's top the second header is the one held, never the first.
        assert_eq!(l.pinned(b.top + 50.0), Some((1, b.top + 50.0)));
        assert_eq!(l.pinned(b.top), None, "the second header in place");
    }

    #[test]
    fn the_slots_hold_the_cells_in_view_wherever_the_grid_scrolls() {
        let groups = vec![("A".to_string(), String::new(), 37), ("B".to_string(), String::new(), 5), ("C".to_string(), String::new(), 600)];
        let l = Layout::new(&groups, 6, 1000.0, Metrics::GALLERY);
        let viewport = 700.0;
        let slots = l.slots(viewport);
        let lens: std::collections::HashSet<usize> = (0..2000).map(|k| l.cells(k as f32 * 7.3, viewport).len()).collect();
        assert!(lens.iter().all(|n| *n <= slots), "{lens:?} within {slots}");
        // Twin: the cells in view vary as the grid scrolls, so a count taken from them would remake tiles.
        assert!(lens.len() > 1, "{lens:?}");
        assert!(slots < l.count / 4, "the slots stay near the view: {slots}");
    }

    fn roll(count: usize) -> Vec<(String, String, usize)> {
        vec![("Roll #8".into(), String::new(), count)]
    }

    #[test]
    fn the_tile_follows_the_width_and_the_cells_sit_on_the_swift_grid() {
        let l = Layout::new(&roll(10), 6, 1000.0, Metrics::GALLERY);
        let tile = (1000.0 - 48.0 - 40.0) / 6.0;
        assert_eq!(l.tile, (tile, tile * 2.0 / 3.0));
        let second_row = l.frame(6).unwrap();
        assert_eq!((second_row.x, second_row.y), (24.0, 24.0 + 37.0 + 8.0 + l.row_pitch()));
        assert_eq!(l.height, 24.0 + 45.0 + 2.0 * l.cell.1 + 8.0 + 24.0);
        assert_eq!(l.index_at(second_row.x + 1.0, second_row.y + 1.0), Some(6));
        // Twin: a point past the last cell clamps to it rather than refusing.
        assert_eq!(l.index_at(990.0, second_row.y + 1.0), Some(9));
    }

    #[test]
    fn a_scroll_asks_only_for_the_rows_in_view() {
        let l = Layout::new(&roll(600), 6, 1000.0, Metrics::GALLERY);
        let pitch = l.row_pitch();
        let top = 30.0 * pitch;
        let range = l.cells(top, 4.0 * pitch);
        assert!(range.len() <= 6 * 7, "{range:?} holds more than the view and its slack");
        assert!(range.contains(&(l.index_at(24.0, top + 1.0).unwrap())));
        // Twin: the whole roll is far more than what one view draws.
        assert!(l.count > 10 * range.len());
        assert_eq!(l.cells(0.0, 0.0), 0..0);
    }
}
