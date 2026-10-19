//! `Blocks`: a measurement read on a grid of cells instead of on pixels. Each cell's median stands
//! for it, so a defect under half a cell never votes and an extreme has to be carried by an area.

use crate::histogram::Histogram;
use crate::plane::Plane;
use crate::window::{contains, contains_point, inset, integral, union};
use oni_settings::axis::pipeline::LUMA_WEIGHTS;
use oni_settings::cg::{Point, Rect};

/// A cell's side in pixels of the measuring copy: 48 rows on a whole frame, as many as a sheet's
/// frame holds, so a cell never shrinks to a pixel.
pub const CELL_SIDE: f64 = 933.0 / 48.0;

/// The share of a cell an extreme has to cover to keep its own value at the ends: a 1 px hair across
/// a 19 px cell is 5.1 %, a 6 × 6 px light is 9.5 % and stays.
pub const TAIL: f32 = 0.08;

/// The bins a reading is counted in.
pub const BINS: usize = 1024;

pub fn rows(height: f64) -> usize {
    ((height / CELL_SIDE).round() as i64).max(1) as usize
}

/// Laid on one window in that image's own pixels, Core Image's frame; row 0 is the top row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub window: Rect,
    pub rows: usize,
    pub columns: usize,
}

impl Grid {
    pub fn new(window: Rect) -> Grid {
        Grid::with_rows(window, rows(window.height))
    }

    pub fn with_rows(window: Rect, rows: usize) -> Grid {
        Grid { window, rows, columns: Grid::columns(rows, window.width / window.height.max(1.0)) }
    }

    pub fn columns(rows: usize, aspect: f64) -> usize {
        ((rows as f64 * aspect).round() as i64).max(1) as usize
    }

    pub fn count(&self) -> usize {
        self.rows * self.columns
    }

    pub fn cell(&self, row: usize, column: usize) -> Rect {
        let w = self.window.width / self.columns as f64;
        let h = self.window.height / self.rows as f64;
        Rect::new(self.window.x + column as f64 * w, self.window.y + self.window.height - (row + 1) as f64 * h, w, h)
    }

    /// As a sheet is read: A1 top-left, letters across, numbers down.
    pub fn name(row: usize, column: usize) -> String {
        let mut letters = Vec::new();
        let mut c = column as i64;
        loop {
            letters.insert(0, (b'A' + (c % 26) as u8) as char);
            c = c / 26 - 1;
            if c < 0 {
                break;
            }
        }
        format!("{}{}", letters.into_iter().collect::<String>(), row + 1)
    }

    pub fn name_of(&self, index: usize) -> String {
        Grid::name(index / self.columns, index % self.columns)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub black: Vec<String>,
    pub white: Vec<String>,
    pub grids: Vec<Grid>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub histogram: Histogram,
    pub report: Option<Report>,
}

/// What one grid says of a counted image, one entry per cell: the medians and the two tails past `TAIL`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cells {
    pub rgb: Vec<[f32; 3]>,
    pub luma: Vec<f32>,
    pub low: Vec<[f32; 3]>,
    pub high: Vec<[f32; 3]>,
}

/// The middle of sorted values; the upper of the two middles on an even count, so a cell half
/// covered by a defect still reads the defect's side.
pub fn median(sorted: &[f32]) -> f32 {
    if sorted.is_empty() { 0.0 } else { sorted[sorted.len() / 2] }
}

/// The value `TAIL` of the sorted values lie beyond, at either end; the depth is figured in f32.
pub fn tail(sorted: &[f32], low: bool) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let depth = ((TAIL * sorted.len() as f32) as usize).min(sorted.len() - 1);
    if low { sorted[depth] } else { sorted[sorted.len() - 1 - depth] }
}

/// The low tail, the median and the high tail `tail` and `median` read on the sorted values, found
/// by selection: three ranks of a cell, never the whole cell sorted.
pub fn ranked(values: &mut [f32]) -> (f32, f32, f32) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0, 0.0);
    }
    let depth = ((TAIL * n as f32) as usize).min(n - 1);
    let mid = n / 2;
    let (left, m, right) = values.select_nth_unstable_by(mid, f32::total_cmp);
    let m = *m;
    let mut at = |rank: usize| match rank.cmp(&mid) {
        std::cmp::Ordering::Less => *left.select_nth_unstable_by(rank, f32::total_cmp).1,
        std::cmp::Ordering::Equal => m,
        std::cmp::Ordering::Greater => *right.select_nth_unstable_by(rank - mid - 1, f32::total_cmp).1,
    };
    let low = at(depth);
    let high = at(n - 1 - depth);
    (low, m, high)
}

/// `Blocks.medians`: the window's box read once, then each row of cells on its own core.
pub fn medians(counted: &Plane, grid: &Grid) -> Cells {
    let rect = integral(&grid.window);
    let (width, height, raw) = counted.bitmap(&rect);
    if width == 0 || height == 0 {
        return Cells::default();
    }
    let edge = |i: usize, span: f64, parts: usize| ((i as f64 * span / parts as f64).floor()) as usize;
    let w = LUMA_WEIGHTS;
    let columns = grid.columns;
    let row_cells = |row: usize| -> Vec<([f32; 3], f32, [f32; 3], [f32; 3])> {
        let y0 = edge(row, rect.height, grid.rows);
        let y1 = edge(row + 1, rect.height, grid.rows).max(y0 + 1).min(height);
        let (mut r, mut g, mut b, mut y) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        (0..columns)
            .map(|column| {
                let x0 = edge(column, rect.width, columns);
                let x1 = edge(column + 1, rect.width, columns).max(x0 + 1).min(width);
                r.clear();
                g.clear();
                b.clear();
                y.clear();
                for py in y0..y1 {
                    for px in x0..x1 {
                        let p = raw[py * width + px];
                        r.push(p[0]);
                        g.push(p[1]);
                        b.push(p[2]);
                        y.push(p[0] * w[0] + p[1] * w[1] + p[2] * w[2]);
                    }
                }
                let (rl, rm, rh) = ranked(&mut r);
                let (gl, gm, gh) = ranked(&mut g);
                let (bl, bm, bh) = ranked(&mut b);
                let mid = y.len() / 2;
                let luma = if y.is_empty() { 0.0 } else { *y.select_nth_unstable_by(mid, f32::total_cmp).1 };
                ([rm, gm, bm], luma, [rl, gl, bl], [rh, gh, bh])
            })
            .collect()
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(grid.rows).max(1);
    let per = grid.rows.div_ceil(threads);
    let rows: Vec<Vec<_>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let row_cells = &row_cells;
                scope.spawn(move || (t * per..((t + 1) * per).min(grid.rows)).map(row_cells).collect::<Vec<_>>())
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a row of cells")).collect()
    });
    let mut cells = Cells::default();
    for (rgb, luma, low, high) in rows.into_iter().flatten() {
        cells.rgb.push(rgb);
        cells.luma.push(luma);
        cells.low.push(low);
        cells.high.push(high);
    }
    cells
}

/// Each cell's median counted in `bins`; an index is truncated toward zero, as Swift's `Int` is.
pub fn histogram(rgb: &[[f32; 3]], luma: &[f32], bins: usize) -> Histogram {
    let mut counts = vec![[0.0f32; 3]; bins];
    let mut trace = vec![0.0f32; bins];
    let bin = |v: f32| ((v * bins as f32) as i64).clamp(0, bins as i64 - 1) as usize;
    for cell in rgb {
        for c in 0..3 {
            counts[bin(cell[c])][c] += 1.0;
        }
    }
    for &l in luma {
        trace[bin(l)] += 1.0;
    }
    Histogram { rgb: counts, luma: trace, ends: None }
}

/// The first of the least, as Swift's `min(by:)` keeps it.
fn first_min(kept: &[usize], key: impl Fn(usize) -> f32) -> usize {
    kept.iter().copied().fold(kept[0], |best, i| if key(i) < key(best) { i } else { best })
}

/// The first of the greatest, as Swift's `max(by:)` keeps it.
fn first_max(kept: &[usize], key: impl Fn(usize) -> f32) -> usize {
    kept.iter().copied().fold(kept[0], |best, i| if key(best) < key(i) { i } else { best })
}

/// `Blocks.reading`: one grid over the windows' bounds, read in one pass, the cells a window wholly
/// holds kept, so a sheet's gutters and borders never vote.
pub fn reading(counted: &Plane, windows: &[Rect], bins: usize) -> Reading {
    let Some(first) = windows.first() else { return Reading::default() };
    let grid = Grid::new(windows[1..].iter().fold(*first, |a, b| union(&a, b)));
    let cells = medians(counted, &grid);
    let rects: Vec<Rect> = (0..cells.rgb.len()).map(|i| grid.cell(i / grid.columns, i % grid.columns)).collect();
    let mut kept: Vec<usize> = (0..rects.len()).filter(|&i| windows.iter().any(|w| contains(&inset(w, -0.5, -0.5), &rects[i]))).collect();
    // A window narrower than a cell still reads: the cells whose centre it holds.
    if kept.is_empty() {
        kept = (0..rects.len())
            .filter(|&i| {
                let centre = Point::new(rects[i].x + rects[i].width / 2.0, rects[i].y + rects[i].height / 2.0);
                windows.iter().any(|w| contains_point(w, centre))
            })
            .collect();
    }
    if kept.is_empty() {
        return Reading::default();
    }
    let (mut black, mut white, mut ends) = (Vec::new(), Vec::new(), Vec::new());
    for c in 0..3 {
        let darkest = first_min(&kept, |i| cells.low[i][c]);
        let lightest = first_max(&kept, |i| cells.high[i][c]);
        black.push(grid.name_of(darkest));
        white.push(grid.name_of(lightest));
        ends.push([cells.low[darkest][c], cells.high[lightest][c]]);
    }
    let rgb: Vec<[f32; 3]> = kept.iter().map(|&i| cells.rgb[i]).collect();
    let luma: Vec<f32> = kept.iter().map(|&i| cells.luma[i]).collect();
    let mut counted = histogram(&rgb, &luma, bins);
    counted.ends = Some(ends);
    Reading { histogram: counted, report: Some(Report { black, white, grids: vec![grid] }) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_median_takes_the_upper_middle_and_the_tail_its_share_of_the_cell() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 3.0);
        assert_eq!(median(&[]), 0.0);
        let sorted: Vec<f32> = (0..25).map(|i| i as f32).collect();
        // 0.08 of 25 is 2 in f32, so the tail sits two values in from each end.
        assert_eq!((tail(&sorted, true), tail(&sorted, false)), (2.0, 22.0));
        // Twin: a cell of 12 keeps its extremes, 0.96 truncating to none.
        let short: Vec<f32> = (0..12).map(|i| i as f32).collect();
        assert_eq!((tail(&short, true), tail(&short, false)), (0.0, 11.0));
    }

    #[test]
    fn a_cell_read_by_selection_reads_what_the_sorted_cell_reads() {
        let mut seed = 7u32;
        for n in 1..60 {
            let values: Vec<f32> = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    // Repeats on purpose: a cell of a flat area holds many equal values.
                    (seed >> 28) as f32 / 4.0
                })
                .collect();
            let mut sorted = values.clone();
            sorted.sort_unstable_by(f32::total_cmp);
            let mut scrambled = values.clone();
            assert_eq!(ranked(&mut scrambled), (tail(&sorted, true), median(&sorted), tail(&sorted, false)), "{n} values");
            // Twin: the lower of two middles is not what a cell reads.
            if n % 2 == 0 && sorted[n / 2 - 1] != sorted[n / 2] {
                assert_ne!(ranked(&mut values.clone()).1, sorted[n / 2 - 1]);
            }
        }
    }
}
