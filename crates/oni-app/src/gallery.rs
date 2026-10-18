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
    /// The gallery's own, `GalleryView.metrics`.
    pub const GALLERY: Metrics = Metrics { margin: 24.0, gutter: 8.0, padding: 24.0, header: 37.0, caption: 14.0, caption_gap: 4.0 };
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
