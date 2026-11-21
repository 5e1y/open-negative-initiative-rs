//! The strokes of a search's own gesture still awaiting their patch: their pixels are dust, left out of what the
//! search matches on, measures and copies.

use super::cost::Candidate;
use super::Region;
use oni_pipeline::corrections;
use oni_settings::frame::Correction;

/// The least of a sample's four taps' weight that must fall on pixels awaiting no patch.
const CLEAR: f32 = 0.999;

pub(super) fn distance(x: f64, y: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l <= 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
    (x - (a.0 + t * dx)).hypot(y - (a.1 + t * dy))
}

/// The cheapest candidate whose copy holds no pixel awaiting its own patch.
pub(super) fn lowest(c: &[Candidate], src: &Region, stroke: &Correction) -> Option<usize> {
    let mut finite: Vec<usize> = (0..c.len()).filter(|&i| c[i].cost.is_finite()).collect();
    if src.awaited.is_empty() {
        return finite.into_iter().min_by(|&a, &b| c[a].cost.total_cmp(&c[b].cost));
    }
    finite.sort_by(|&a, &b| c[a].cost.total_cmp(&c[b].cost));
    finite.into_iter().find(|&i| !src.copies_awaited(stroke, c[i].offset))
}

impl Region {
    /// Whether the pixel `texel` reads awaits a patch.
    pub(super) fn awaits(&self, col: i64, row_up: i64) -> bool {
        if self.awaited.is_empty() || col < 0 || row_up < 0 || col as f64 >= self.frame_width || row_up as f64 >= self.frame_height {
            return false;
        }
        let row_down = self.frame_height as i64 - 1 - row_up;
        let c = (col - self.x).clamp(0, self.width as i64 - 1) as usize;
        let r = (row_down - self.y).clamp(0, self.height as i64 - 1) as usize;
        self.awaited[r * self.width + c]
    }

    /// Whether `sample` at this point reads no pixel awaiting a patch: its four taps' share of them under a thousandth.
    pub(super) fn clear(&self, x: f64, y: f64) -> bool {
        if self.awaited.is_empty() {
            return true;
        }
        let (u, v) = (x - 0.5, y - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let open = |col, row| if self.awaits(col, row) { 0.0f32 } else { 1.0 };
        (open(x0, y0) * (1.0 - fx) + open(x0 + 1, y0) * fx) * (1.0 - fy) + (open(x0, y0 + 1) * (1.0 - fx) + open(x0 + 1, y0 + 1) * fx) * fy >= CLEAR
    }

    /// The region with the strokes of `awaiting`, kept on a scan of this frame's size, laid on `awaited`: each one's brush along its path.
    pub fn awaiting<'a>(mut self, awaiting: impl Iterator<Item = &'a Correction>) -> Region {
        for c in awaiting {
            let mut under = Vec::new();
            self.under(c, c.radius, (0.0, 0.0), |i| {
                under.push(i);
                false
            });
            if self.awaited.is_empty() && !under.is_empty() {
                self.awaited = vec![false; self.pixels.len()];
            }
            for i in under {
                self.awaited[i] = true;
            }
        }
        self
    }

    /// Each of the region's pixels within `reach` of the stroke of `c` moved by `offset`, y down, until `each` stops; true once stopped.
    fn under(&self, c: &Correction, reach: f64, offset: (f64, f64), mut each: impl FnMut(usize) -> bool) -> bool {
        let points: Vec<(f64, f64)> = c.stroke.iter().map(|p| (p.x * self.frame_width, p.y * self.frame_height)).collect();
        let Some(&first) = points.first() else { return false };
        let b = points.iter().fold([first.0, first.1, first.0, first.1], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
        let near = |x: f64, y: f64| points.windows(2).map(|w| distance(x, y, w[0], w[1])).fold((x - first.0).hypot(y - first.1), f64::min) <= reach;
        let span = |low: f64, high: f64, moved: f64, origin: i64, side: usize| {
            let (from, to) = ((low - reach + moved).floor() as i64 - origin, (high + reach + moved).ceil() as i64 - origin + 1);
            from.max(0)..to.min(side as i64)
        };
        for row in span(b[1], b[3], offset.1, self.y, self.height) {
            for col in span(b[0], b[2], offset.0, self.x, self.width) {
                let (x, y) = ((col + self.x) as f64 + 0.5 - offset.0, (row + self.y) as f64 + 0.5 - offset.1);
                if near(x, y) && each(row as usize * self.width + col as usize) {
                    return true;
                }
            }
        }
        false
    }

    /// Whether the patch of `c` copied from `offset` away, y up, would hold a pixel awaiting its own patch:
    /// under its stroke, its feathered seam and a pixel for the offset's rounding.
    fn copies_awaited(&self, c: &Correction, offset: (f64, f64)) -> bool {
        !self.awaited.is_empty() && self.under(c, c.radius + 3.0 * corrections::feather(c, 1.0) + 1.0, (offset.0, -offset.1), |i| self.awaited[i])
    }
}
