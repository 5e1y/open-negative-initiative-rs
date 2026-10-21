//! Edit Together's sheet: tiles reduced to a cell and laid six to a row, at the share of its size shown.

use super::*;

/// `linear` averaged down to `side` on its longest side, each output pixel the mean of its footprint.
pub(super) fn reduced(linear: render::Linear, side: usize) -> render::Linear {
    let (w, h) = (linear.width, linear.height);
    let long = w.max(h);
    if long <= side {
        return linear;
    }
    let (ow, oh) = ((w * side).div_ceil(long).max(1), (h * side).div_ceil(long).max(1));
    let mut pixels = Vec::with_capacity(ow * oh);
    for y in 0..oh {
        let (y0, y1) = (y * h / oh, ((y + 1) * h / oh).max(y * h / oh + 1));
        for x in 0..ow {
            let (x0, x1) = (x * w / ow, ((x + 1) * w / ow).max(x * w / ow + 1));
            let mut sum = [0.0f32; 4];
            for row in y0..y1 {
                for p in &linear.pixels[row * w + x0..row * w + x1] {
                    (0..4).for_each(|c| sum[c] += p[c]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as f32;
            pixels.push(sum.map(|v| v / n));
        }
    }
    render::Linear { width: ow, height: oh, pixels }
}

/// A cell's side and the gap after it, in the sheet's pixels, at `scale` of the full sheet.
pub(super) fn cell_at(scale: f64) -> (usize, usize) {
    (((TILE as f64 * scale).round() as usize).max(1), (GAP as f64 * scale).round() as usize)
}

/// A sheet of `count` tiles six to a row, with the gaps between them.
pub(super) fn sheet_size(count: usize, (tile, gap): (usize, usize)) -> (u32, u32) {
    let columns = count.clamp(1, COLUMNS);
    let rows = count.div_ceil(COLUMNS).max(1);
    ((columns * tile + (columns - 1) * gap) as u32, (rows * tile + (rows - 1) * gap) as u32)
}

/// The corner of tile `i` of `size`, centred in its cell.
pub(super) fn place(i: usize, size: (u32, u32), (tile, gap): (usize, usize)) -> (i32, i32) {
    let (cx, cy) = ((i % COLUMNS) * (tile + gap), (i / COLUMNS) * (tile + gap));
    let (w, h) = (size.0 as usize, size.1 as usize);
    ((cx + tile.saturating_sub(w) / 2) as i32, (cy + tile.saturating_sub(h) / 2) as i32)
}

/// A sheet's rectangle in its pixels: x, y, width, height.
pub(super) type Window = (u32, u32, u32, u32);

/// Which of `count` tiles a view showing `visible` of the sheet meets, and the cells' box around them,
/// so a grid draws the tiles on screen alone, on a sheet no larger than they need.
pub(super) fn seen(count: usize, (tile, gap): (usize, usize), visible: (f64, f64, f64, f64)) -> (Vec<bool>, Window) {
    let (vx0, vy0, vx1, vy1) = (visible.0, visible.1, visible.0 + visible.2, visible.1 + visible.3);
    let mut corners = (u32::MAX, u32::MAX, 0, 0);
    let shows = (0..count)
        .map(|i| {
            let (x, y) = ((i % COLUMNS) * (tile + gap), (i / COLUMNS) * (tile + gap));
            let (x1, y1) = (x + tile, y + tile);
            let meets = (x as f64) < vx1 && (x1 as f64) > vx0 && (y as f64) < vy1 && (y1 as f64) > vy0;
            if meets {
                corners = (corners.0.min(x as u32), corners.1.min(y as u32), corners.2.max(x1 as u32), corners.3.max(y1 as u32));
            }
            meets
        })
        .collect::<Vec<_>>();
    let whole = sheet_size(count, (tile, gap));
    let window = if corners.0 == u32::MAX { (0, 0, whole.0, whole.1) } else { (corners.0, corners.1, corners.2 - corners.0, corners.3 - corners.1) };
    (shows, window)
}

/// The share of the full sheet worth drawing for a view showing it `shown` wide in screen pixels:
/// never past the full sheet, in steps of an eighth so a zoom redraws only when it crosses one.
pub(super) fn scale_for(shown: f64, full: f64) -> f64 {
    ((shown / full.max(1.0) * 8.0).ceil() / 8.0).clamp(0.125, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_draws_the_tiles_its_view_meets_on_their_box_alone() {
        let cell = cell_at(1.0);
        // Zoomed on the second tile of the second row: it alone, on its own cell.
        let at = (TILE + GAP) as f64;
        let (shows, window) = seen(12, cell, (at + 10.0, at + 10.0, 300.0, 200.0));
        assert_eq!(shows.iter().filter(|s| **s).count(), 1);
        assert!(shows[7] && !shows[0], "twin: a tile off the view is not drawn");
        assert_eq!(window, ((TILE + GAP) as u32, (TILE + GAP) as u32, TILE as u32, TILE as u32));
        // The whole sheet in view: every tile, the whole sheet.
        let whole = sheet_size(12, cell);
        let (shows, window) = seen(12, cell, (0.0, 0.0, f64::from(whole.0), f64::from(whole.1)));
        assert!(shows.iter().all(|s| *s));
        assert_eq!(window, (0, 0, whole.0, whole.1));
    }

    #[test]
    fn a_reduction_keeps_the_mean_and_the_longest_side() {
        let pixels = (0..2 * TILE * TILE).map(|i| if i % 2 == 0 { [1.0, 0.0, 0.5, 1.0] } else { [0.0, 1.0, 0.5, 1.0] }).collect();
        let r = reduced(render::Linear { width: 2 * TILE, height: TILE, pixels }, TILE);
        assert_eq!((r.width, r.height), (TILE, TILE / 2));
        assert!(r.pixels.iter().all(|p| (p[0] - 0.5).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6), "{:?}", r.pixels[0]);
        // Twin: a picture already small is kept as it is, not averaged.
        let small = reduced(render::Linear { width: 2, height: 1, pixels: vec![[1.0; 4], [0.0; 4]] }, TILE);
        assert_eq!(small.pixels, vec![[1.0; 4], [0.0; 4]]);
    }

    #[test]
    fn seven_tiles_lay_two_rows_of_six_each_centred_in_its_cell() {
        let full = cell_at(1.0);
        assert_eq!(sheet_size(7, full), ((6 * TILE + 5 * GAP) as u32, (2 * TILE + GAP) as u32));
        assert_eq!(sheet_size(3, full), ((3 * TILE + 2 * GAP) as u32, TILE as u32), "a short row is only as wide as its tiles");
        let (long, short) = (TILE as u32, TILE as u32 / 2);
        let inset = (TILE / 4) as i32;
        assert_eq!(place(0, (long, short), full), (0, inset), "a landscape tile centred in its cell's height");
        assert_eq!(place(6, (short, long), full), (inset, (TILE + GAP) as i32), "the seventh opens the second row");
        // Twin: the second tile's cell starts past the first's and the gap.
        assert_eq!(place(1, (long, long), full), ((TILE + GAP) as i32, 0));
        // At half the scale, half the cells.
        assert_eq!(sheet_size(7, cell_at(0.5)), ((6 * TILE / 2 + 5 * GAP / 2) as u32, (TILE + GAP / 2) as u32));
    }

    #[test]
    fn a_sheet_is_drawn_at_the_size_it_is_shown_in_eighths_never_past_its_own() {
        assert_eq!(scale_for(1230.0, 2460.0), 0.5);
        assert_eq!(scale_for(1231.0, 2460.0), 0.625, "a pixel past a step draws the next one");
        assert_eq!(scale_for(9000.0, 2460.0), 1.0, "zoomed past the sheet: its own pixels, no more");
        // Twin: a tiny view still draws an eighth rather than nothing.
        assert_eq!(scale_for(10.0, 2460.0), 0.125);
    }
}
