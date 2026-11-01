//! Edit Together's sheet: each frame framed to a tile and laid six to a row, at the share of its size shown.

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

/// The longest side a frame's source is reduced to, so that its crop still spans `TILE` pixels: a
/// frame cropped to half its width keeps twice as many.
pub(super) fn source_side(settings: &PipelineSettings, size: (usize, usize)) -> usize {
    let (w, h) = (size.0 as f64, size.1 as f64);
    let (cw, ch) = crate::export::cropped_size((w, h), settings);
    (TILE as f64 * w.max(h) / cw.max(ch).max(1.0)).ceil() as usize
}

/// A rectangle on the full sheet: x, y, width, height.
pub(super) type Area = (f64, f64, f64, f64);

/// A sheet's rectangle in its drawn pixels: x, y, width, height.
pub(super) type Window = (u32, u32, u32, u32);

/// Pixels a drawn box leaves around its tiles: the pipeline rounds a crop's edges outward, and a
/// tile one pixel past its place would not be drawn.
const SLACK: f64 = 2.0;

/// A sheet laid at full scale: each picture's place and its size, and the sheet's, margin included.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Sheet {
    pub pictures: Vec<Area>,
    pub size: (f64, f64),
}

/// Frames of these proportions six to a row, each `TILE` on its longest side whatever its crop: a
/// column as wide as a tile, a row as tall as its tallest picture and each picture centred in both,
/// `GAP` between columns, `ROWS_APART` between rows and `MARGIN` around the sheet, its last numbers' band.
pub(super) fn laid(proportions: &[(f64, f64)]) -> Sheet {
    let (tile, gap, apart, margin) = (TILE as f64, GAP as f64, ROWS_APART as f64, MARGIN as f64);
    let fitted: Vec<(f64, f64)> = proportions.iter().map(|&(w, h)| (tile * w / w.max(h).max(1e-9), tile * h / w.max(h).max(1e-9))).collect();
    let columns = fitted.len().clamp(1, COLUMNS) as f64;
    let mut pictures = Vec::with_capacity(fitted.len());
    let mut y = margin;
    for row in fitted.chunks(COLUMNS) {
        let height = row.iter().map(|p| p.1).fold(0.0, f64::max);
        for (c, &(w, h)) in row.iter().enumerate() {
            pictures.push((margin + c as f64 * (tile + gap) + (tile - w) / 2.0, y + (height - h) / 2.0, w, h));
        }
        y += height + apart;
    }
    // The last row is followed by the margin, not the rows' spacing.
    let bottom = if pictures.is_empty() { 2.0 * margin } else { y - apart + margin };
    let width = 2.0 * margin + columns * tile + (columns - 1.0) * gap;
    Sheet { pictures, size: (width, bottom) }
}

impl Sheet {
    /// Its size drawn at `scale`, in whole pixels.
    pub fn drawn(&self, scale: f64) -> (u32, u32) {
        ((self.size.0 * scale).ceil() as u32, (self.size.1 * scale).ceil() as u32)
    }

    /// The corner of tile `i` drawn at `scale`, a picture of `size` centred on its place.
    pub fn place(&self, i: usize, size: (u32, u32), scale: f64) -> (i32, i32) {
        let (x, y, w, h) = self.pictures[i];
        let at = |v: f64, length: f64, drawn: u32| (v * scale + (length * scale - f64::from(drawn)) / 2.0).round() as i32;
        (at(x, w, size.0), at(y, h, size.1))
    }

    /// Which tiles a view showing `visible` of the sheet drawn at `scale` meets, and the box around
    /// them there, so a grid draws the tiles on screen alone, on a sheet no larger than they need.
    pub fn seen(&self, scale: f64, visible: Area) -> (Vec<bool>, Window) {
        let (vx0, vy0, vx1, vy1) = (visible.0, visible.1, visible.0 + visible.2, visible.1 + visible.3);
        let mut corners = (f64::MAX, f64::MAX, 0.0f64, 0.0f64);
        let shows = self
            .pictures
            .iter()
            .map(|&(x, y, w, h)| {
                let (x0, y0, x1, y1) = (x * scale, y * scale, (x + w) * scale, (y + h) * scale);
                let meets = x0 < vx1 && x1 > vx0 && y0 < vy1 && y1 > vy0;
                if meets {
                    corners = (corners.0.min(x0), corners.1.min(y0), corners.2.max(x1), corners.3.max(y1));
                }
                meets
            })
            .collect();
        let whole = self.drawn(scale);
        if corners.0 == f64::MAX {
            return (shows, (0, 0, whole.0, whole.1));
        }
        let (x0, y0) = ((corners.0 - SLACK).floor().max(0.0) as u32, (corners.1 - SLACK).floor().max(0.0) as u32);
        let (x1, y1) = (((corners.2 + SLACK).ceil() as u32).min(whole.0), ((corners.3 + SLACK).ceil() as u32).min(whole.1));
        (shows, (x0, y0, x1 - x0, y1 - y0))
    }
}

/// The share of the full sheet worth drawing for a view showing it `shown` wide in screen pixels:
/// never past the full sheet, in steps of an eighth so a zoom redraws only when it crosses one.
pub(super) fn scale_for(shown: f64, full: f64) -> f64 {
    ((shown / full.max(1.0) * 8.0).ceil() / 8.0).clamp(0.125, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANDSCAPE: (f64, f64) = (3.0, 2.0);
    const PORTRAIT: (f64, f64) = (2.0, 3.0);

    #[test]
    fn a_grid_draws_the_tiles_its_view_meets_on_their_box_alone() {
        let sheet = laid(&[LANDSCAPE; 12]);
        let (x, y, w, h) = sheet.pictures[7];
        // Zoomed on the second tile of the second row: it alone, on its own box.
        let (shows, window) = sheet.seen(1.0, (x + 10.0, y + 10.0, 300.0, 200.0));
        assert_eq!(shows.iter().filter(|s| **s).count(), 1);
        assert!(shows[7] && !shows[0], "twin: a tile off the view is not drawn");
        let (x0, y0) = ((x - SLACK).floor(), (y - SLACK).floor());
        assert_eq!(window, (x0 as u32, y0 as u32, ((x + w + SLACK).ceil() - x0) as u32, ((y + h + SLACK).ceil() - y0) as u32));
        // A tile a pixel larger than its place, as a rounded crop renders, still lies inside the box.
        let (px, py) = sheet.place(7, ((w + 1.0) as u32, (h + 1.0) as u32), 1.0);
        assert!(px >= window.0 as i32 && py >= window.1 as i32 && px + w as i32 + 1 <= (window.0 + window.2) as i32 && py + h as i32 + 1 <= (window.1 + window.3) as i32);
        // The whole sheet in view: every tile.
        let whole = sheet.drawn(1.0);
        let (shows, _) = sheet.seen(1.0, (0.0, 0.0, f64::from(whole.0), f64::from(whole.1)));
        assert!(shows.iter().all(|s| *s));
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
    fn a_crop_spans_a_tile_landscape_across_and_portrait_down_its_rows_as_tall_as_their_tallest() {
        let (tile, gap, apart, margin) = (TILE as f64, GAP as f64, ROWS_APART as f64, MARGIN as f64);
        // Six landscape frames, the third cropped tight, then a portrait one opening the second row.
        let mut frames = vec![LANDSCAPE; 6];
        frames[2] = (300.0, 200.0);
        frames.push(PORTRAIT);
        let sheet = laid(&frames);
        let low = tile * 2.0 / 3.0;
        assert_eq!(sheet.pictures[0], (margin, margin, tile, low), "a landscape frame is a tile wide");
        assert_eq!(sheet.pictures[2].2, tile, "a tight crop as wide as any other");
        let (x, y, w, h) = sheet.pictures[6];
        assert_eq!((w, h), (low, tile), "a portrait frame is a tile tall");
        assert_eq!((x, y), (margin + (tile - low) / 2.0, margin + low + apart), "under the first row, four columns' gaps apart");
        assert_eq!(apart, 2.0 * margin, "twin: the rows stand twice as far apart as the margin");
        assert_eq!(sheet.size, (2.0 * margin + 6.0 * tile + 5.0 * gap, margin + low + apart + tile + margin));
        // Twin: a portrait frame in the first row makes it as tall as itself, the landscapes centred in it.
        frames[1] = PORTRAIT;
        let taller = laid(&frames);
        assert_eq!(taller.pictures[0].1, margin + (tile - low) / 2.0);
        assert_eq!(taller.pictures[6].1, margin + tile + apart, "the second row moves down with it");
    }

    #[test]
    fn a_tile_is_placed_centred_on_its_picture_at_the_scale_drawn() {
        let sheet = laid(&[LANDSCAPE, (1.0, 2.0)]);
        let (x, y, w, h) = sheet.pictures[1];
        assert_eq!((w, h), (TILE as f64 / 2.0, TILE as f64));
        assert_eq!(sheet.place(1, (256, 512), 0.5), ((x * 0.5) as i32, (y * 0.5) as i32));
        // Twin: a tile drawn two pixels short of its place sits a pixel in, not at its corner.
        assert_eq!(sheet.place(1, (254, 512), 0.5).0, (x * 0.5) as i32 + 1);
    }

    #[test]
    fn a_source_keeps_enough_pixels_for_its_crop_to_span_a_tile() {
        let mut s = PipelineSettings::default();
        assert_eq!(source_side(&s, (6000, 4000)), TILE);
        (s.geometry.crop.width, s.geometry.crop.height) = (0.5, 0.5);
        assert_eq!(source_side(&s, (6000, 4000)), 2 * TILE, "half of each side kept: twice the pixels");
        // Twin: a crop of the height alone leaves the longest side, and the source, as they were.
        s.geometry.crop.width = 1.0;
        s.geometry.crop.height = 0.5;
        assert_eq!(source_side(&s, (6000, 4000)), TILE);
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
