//! `CorrectionRenderer.generate`: a dust correction's patch, copied from the place on the frame whose
//! surroundings match the stroke's best, its tone matched by a bias varying across it. The search
//! runs in Core Image's frame, y up, as Swift's kernels do; rectangles come in and go out y down.

use oni_pipeline::corrections::{self, Area};
use oni_settings::frame::Correction;

/// A region of the framed frame read back to the CPU: its rectangle in the frame, y down, and its pixels.
pub struct Region {
    pub x: i64,
    pub y: i64,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
    /// The framed frame's own height, which turns a row down into Core Image's y up.
    pub frame_height: f64,
}

impl Region {
    /// The pixel whose centre-anchored square holds `(x, y)`, y up, edge-clamped inside the region.
    fn texel(&self, col: i64, row_up: i64) -> [f32; 4] {
        let row_down = self.frame_height as i64 - 1 - row_up;
        let c = (col - self.x).clamp(0, self.width as i64 - 1) as usize;
        let r = (row_down - self.y).clamp(0, self.height as i64 - 1) as usize;
        self.pixels[r * self.width + c]
    }

    /// `coreimage::sample`: bilinear, texel centres on the half pixel.
    fn sample(&self, x: f64, y: f64) -> [f32; 3] {
        let (u, v) = (x - 0.5, y - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(x0, y0);
        let b = self.texel(x0 + 1, y0);
        let c = self.texel(x0, y0 + 1);
        let d = self.texel(x0 + 1, y0 + 1);
        std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    }

    /// `CIAreaAverage` over a rectangle, y up: the pixels whose centres fall inside it.
    fn mean(&self, r: Rect) -> Option<[f32; 3]> {
        let (x0, x1) = ((r.x - 0.5).ceil() as i64, (r.x + r.w - 0.5).floor() as i64);
        let (y0, y1) = ((r.y - 0.5).ceil() as i64, (r.y + r.h - 0.5).floor() as i64);
        if x1 < x0 || y1 < y0 {
            return None;
        }
        let mut sum = [0.0f64; 3];
        for row in y0..=y1 {
            for col in x0..=x1 {
                let p = self.texel(col, row);
                for k in 0..3 {
                    sum[k] += f64::from(p[k]);
                }
            }
        }
        let n = ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64;
        Some(sum.map(|s| (s / n) as f32))
    }
}

/// A rectangle in Core Image's frame, y up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn up(a: Area, height: f64) -> Rect {
        Rect { x: a[0], y: height - a[1] - a[3], w: a[2], h: a[3] }
    }

    fn intersection(self, o: Rect) -> Rect {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = ((self.x + self.w).min(o.x + o.w), (self.y + self.h).min(o.y + o.h));
        Rect { x: x0, y: y0, w: (x1 - x0).max(0.0), h: (y1 - y0).max(0.0) }
    }

    fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

/// The frame's extent and the correction's places in it, y up.
struct Frame {
    extent: Rect,
    tile: Rect,
    centre: (f64, f64),
    hole: Rect,
}

fn frame(c: &Correction, width: f64, height: f64) -> Option<Frame> {
    let extent = [0.0, 0.0, width, height];
    let tile = Rect::up(corrections::context_rect(c, extent), height);
    let hole = Rect::up(corrections::stroke_bounding_rect(c, extent)?, height);
    let xs = c.stroke.iter().map(|p| p.x * width);
    let ys = c.stroke.iter().map(|p| (1.0 - p.y) * height);
    let (x0, x1) = xs.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let (y0, y1) = ys.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    Some(Frame { extent: Rect { x: 0.0, y: 0.0, w: width, h: height }, tile, centre: ((x0 + x1) / 2.0, (y0 + y1) / 2.0), hole })
}

/// The largest half-extent keeping `centre ± half` inside the extent.
fn edge_clamped_half(desired: (f64, f64), centre: (f64, f64), e: Rect) -> (f64, f64) {
    let hw = (centre.0 - e.x).min(e.x + e.w - centre.0).max(0.0);
    let hh = (centre.1 - e.y).min(e.y + e.h - centre.1).max(0.0);
    (desired.0.min(hw), desired.1.min(hh))
}

/// `contextRing`: the template the search and the tone match share, three holes wide.
fn context_ring(centre: (f64, f64), hole: Rect, e: Rect) -> Rect {
    let desired = (hole.w.max(hole.h) * 3.0).min(e.w.min(e.h));
    let half = edge_clamped_half((desired / 2.0, desired / 2.0), centre, e);
    let side = half.0.min(half.1) * 2.0;
    Rect { x: centre.0 - side / 2.0, y: centre.1 - side / 2.0, w: side, h: side }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    offset: (f64, f64),
    cost: f32,
}

/// `correctionCost` over a grid of candidate offsets, row-major from the bottom as Core Image
/// dispatches it: a ring's cells sampled at each, the hole left out on either side.
#[allow(clippy::too_many_arguments)]
fn cost_map(src: &Region, f: &Frame, half: (f64, f64), origin: (f64, f64), step: f64, grid_n: usize, size: usize, min_radius: f64) -> Vec<Candidate> {
    let e = f.extent;
    let hole = f.hole;
    let inside = |p: (f64, f64)| p.0 >= hole.x && p.0 <= hole.x + hole.w && p.1 >= hole.y && p.1 <= hole.y + hole.h;
    let cell = |j: usize| (j as f64 + 0.5) / grid_n as f64 * 2.0 - 1.0;
    let one = |index: usize| {
        let (i, j) = (index % size, index / size);
        let offset = (origin.0 + (i as f64 + 0.5) * step, origin.1 + (j as f64 + 0.5) * step);
        let cand = (f.centre.0 + offset.0, f.centre.1 + offset.1);
        if offset.0.hypot(offset.1) < min_radius
            || cand.0 - half.0 < e.x
            || cand.0 + half.0 > e.x + e.w
            || cand.1 - half.1 < e.y
            || cand.1 + half.1 > e.y + e.h
        {
            return Candidate { offset, cost: f32::INFINITY };
        }
        let (mut accum, mut counted) = ([0.0f32; 3], 0usize);
        for cj in 0..grid_n {
            for ci in 0..grid_n {
                let t = (cell(ci) * half.0, cell(cj) * half.1);
                let dest = (f.centre.0 + t.0, f.centre.1 + t.1);
                let from = (cand.0 + t.0, cand.1 + t.1);
                if inside(dest) || inside(from) {
                    continue;
                }
                let d = src.sample(dest.0, dest.1);
                let s = src.sample(from.0, from.1);
                for k in 0..3 {
                    let diff = d[k] - s[k];
                    accum[k] += diff * diff / (d[k] * d[k] + 0.001);
                }
                counted += 1;
            }
        }
        let cost = if counted == 0 { f32::INFINITY } else { (accum[0] + accum[1] + accum[2]) / counted as f32 };
        Candidate { offset, cost }
    };
    let count = size * size;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = count.div_ceil(threads).max(1);
    let mut out = vec![Candidate { offset: (0.0, 0.0), cost: f32::INFINITY }; count];
    std::thread::scope(|scope| {
        for (k, chunk) in out.chunks_mut(per).enumerate() {
            scope.spawn(move || {
                for (n, slot) in chunk.iter_mut().enumerate() {
                    *slot = one(k * per + n);
                }
            });
        }
    });
    out
}

fn lowest(c: &[Candidate]) -> Option<usize> {
    (0..c.len()).filter(|&i| c[i].cost.is_finite()).min_by(|&a, &b| c[a].cost.total_cmp(&c[b].cost))
}

/// `bestSourceRect`'s offset from the tile: a coarse pass over a wide window, then a fine pass at
/// a pixel's step about its best, refined on the parabola and rounded to whole pixels; y up.
pub fn best_offset(src: &Region, c: &Correction, width: f64, height: f64) -> (f64, f64) {
    search(src, c, width, height).0
}

/// The offset, and whether the search found no match and fell back on the tile beside itself.
fn search(src: &Region, c: &Correction, width: f64, height: f64) -> ((f64, f64), bool) {
    let Some(f) = frame(c, width, height) else { return (fallback(&frame_without_stroke(c, width, height)), true) };
    let ring = context_ring(f.centre, f.hole, f.extent);
    let half = (ring.w / 2.0, ring.h / 2.0);
    let min_radius = half.0.max(half.1) * 1.2;
    let coarse_radius = (f.tile.w.max(f.tile.h) * 1.6).max(min_radius * 1.5);
    let (coarse_step, coarse_n) = (8.0, 8usize);
    let coarse_size = (coarse_radius * 2.0 / coarse_step).round() as usize + 1;
    let coarse = cost_map(src, &f, half, (-coarse_radius, -coarse_radius), coarse_step, coarse_n, coarse_size, min_radius);
    let Some(best_coarse) = lowest(&coarse).map(|i| coarse[i]) else { return (fallback(&f), true) };

    let fine_half = edge_clamped_half((30f64.max(f.hole.w * 0.75), 30f64.max(f.hole.h * 0.75)), f.centre, f.extent);
    let fine_n = ((fine_half.0.max(fine_half.1) / 1.25).ceil() as usize).clamp(24, 64);
    let blind = half.0.max(half.1) / coarse_n as f64;
    let fine_radius = blind * 2.0 + coarse_step;
    let fine_size = (fine_radius * 2.0).round() as usize + 1;
    let fine_min = fine_half.0.max(fine_half.1) * 1.2;
    let origin = (best_coarse.offset.0 - fine_radius, best_coarse.offset.1 - fine_radius);
    let fine = cost_map(src, &f, fine_half, origin, 1.0, fine_n, fine_size, fine_min);
    let Some(at) = lowest(&fine) else { return ((best_coarse.offset.0.round(), best_coarse.offset.1.round()), false) };

    let mut best = fine[at].offset;
    let (row, col) = (at / fine_size, at % fine_size);
    if row > 0 && row < fine_size - 1 && col > 0 && col < fine_size - 1 {
        let centre_cost = fine[at].cost;
        let refine = |minus: Candidate, plus: Candidate, current: f64, axis: fn((f64, f64)) -> f64| {
            let denom = minus.cost - 2.0 * centre_cost + plus.cost;
            if !(denom > 0.0 && minus.cost.is_finite() && plus.cost.is_finite()) {
                return current;
            }
            let delta = (0.5 * (minus.cost - plus.cost) / denom).clamp(-0.5, 0.5);
            current + f64::from(delta) * (axis(plus.offset) - axis(minus.offset)) / 2.0
        };
        best.0 = refine(fine[row * fine_size + col - 1], fine[row * fine_size + col + 1], best.0, |o| o.0);
        best.1 = refine(fine[(row - 1) * fine_size + col], fine[(row + 1) * fine_size + col], best.1, |o| o.1);
    }
    ((best.0.round(), best.1.round()), false)
}

fn frame_without_stroke(c: &Correction, width: f64, height: f64) -> Frame {
    let tile = Rect::up(corrections::context_rect(c, [0.0, 0.0, width, height]), height);
    Frame { extent: Rect { x: 0.0, y: 0.0, w: width, h: height }, tile, centre: (0.0, 0.0), hole: tile }
}

/// The tile beside itself, the side with room: what the search falls back to.
fn fallback(f: &Frame) -> (f64, f64) {
    if f.tile.x + 2.0 * f.tile.w > f.extent.x + f.extent.w { (-f.tile.w, 0.0) } else { (f.tile.w, 0.0) }
}

/// `maskedMeanColor`: a rectangle's mean with the hole's share taken out, unless the hole fills it.
fn masked_mean(src: &Region, r: Rect, hole: Rect) -> Option<[f32; 3]> {
    let big = src.mean(r)?;
    let inside = hole.intersection(r);
    if inside.is_empty() {
        return Some(big);
    }
    let Some(hole_mean) = src.mean(inside) else { return Some(big) };
    let (big_area, hole_area) = ((r.w * r.h) as f32, (inside.w * inside.h) as f32);
    let rest = big_area - hole_area;
    if rest <= big_area * 0.3 {
        return Some(big);
    }
    Some(std::array::from_fn(|k| (big[k] * big_area - hole_mean[k] * hole_area) / rest))
}

/// `ToneField`: the bias the copy takes, its value at the stroke's centre and its slope each way, y up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ToneField {
    pub base: [f32; 3],
    pub grad_x: [f32; 3],
    pub grad_y: [f32; 3],
}

fn tone_field(src: &Region, destination: Rect, source: Rect, hole: Rect) -> ToneField {
    let (Some(dm), Some(sm)) = (masked_mean(src, destination, hole), src.mean(source)) else { return ToneField::default() };
    let base: [f32; 3] = std::array::from_fn(|k| dm[k] - sm[k]);
    let left = |r: Rect| Rect { w: r.w / 2.0, ..r };
    let right = |r: Rect| Rect { x: r.x + r.w / 2.0, w: r.w / 2.0, ..r };
    let bottom = |r: Rect| Rect { h: r.h / 2.0, ..r };
    let top = |r: Rect| Rect { y: r.y + r.h / 2.0, h: r.h / 2.0, ..r };
    let means = (
        masked_mean(src, left(destination), hole),
        masked_mean(src, right(destination), hole),
        masked_mean(src, bottom(destination), hole),
        masked_mean(src, top(destination), hole),
        src.mean(left(source)),
        src.mean(right(source)),
        src.mean(bottom(source)),
        src.mean(top(source)),
    );
    let (Some(dl), Some(dr), Some(db), Some(dt), Some(sl), Some(sr), Some(sb), Some(st)) = means else {
        return ToneField { base, ..ToneField::default() };
    };
    let span_x = (destination.w.max(1.0) / 2.0) as f32;
    let span_y = (destination.h.max(1.0) / 2.0) as f32;
    ToneField {
        base,
        grad_x: std::array::from_fn(|k| ((dr[k] - sr[k]) - (dl[k] - sl[k])) / span_x.max(1e-3)),
        grad_y: std::array::from_fn(|k| ((dt[k] - st[k]) - (db[k] - sb[k])) / span_y.max(1e-3)),
    }
}

/// The patch: its rectangle on the frame, y down as the pipeline places it, and its pixels row by
/// row from the top, copied from `offset` away and biased by the tone field.
pub struct Patch {
    pub rect: Area,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
    /// Where the copy was taken, as a fraction of the frame's extent, y down.
    pub offset: (f64, f64),
    /// No match was found: the patch is the plain tile beside the stroke.
    pub fallback: bool,
}

/// `CorrectionRenderer.generate` on a frame `width` × `height` whose region `src` holds the search
/// window and the patch's source, the framed frame with this correction left out.
pub fn generate(src: &Region, c: &Correction, width: f64, height: f64) -> Patch {
    let ((dx, dy), fell_back) = search(src, c, width, height);
    let extent = [0.0, 0.0, width, height];
    let rect = corrections::patch_rect(c, extent);
    let field = frame(c, width, height).map_or(ToneField::default(), |f| {
        let destination = context_ring(f.centre, f.hole, f.extent);
        let source = Rect { x: destination.x + dx, y: destination.y + dy, ..destination };
        tone_field(src, destination, source, f.hole)
    });
    let centre = frame(c, width, height).map_or((0.0, 0.0), |f| f.centre);
    let (w, h) = (rect[2] as usize, rect[3] as usize);
    let mut pixels = Vec::with_capacity(w * h);
    for row in 0..h {
        for col in 0..w {
            // The destination pixel's centre, y up, and the source pixel `offset` away.
            let x = rect[0] + col as f64 + 0.5;
            let y_up = height - (rect[1] + row as f64 + 0.5);
            let s = src.texel((x + dx - 0.5).floor() as i64, (y_up + dy - 0.5).floor() as i64);
            let bias: [f32; 3] = std::array::from_fn(|k| field.base[k] + field.grad_x[k] * (x - centre.0) as f32 + field.grad_y[k] * (y_up - centre.1) as f32);
            pixels.push([s[0] + bias[0], s[1] + bias[1], s[2] + bias[2], s[3]]);
        }
    }
    Patch { rect, width: w, height: h, pixels, offset: (dx / width, -dy / height), fallback: fell_back }
}

/// The rectangle of the framed frame, y down, a search for `c` reads: every candidate's ring and
/// the patch's own source, inside the frame.
pub fn search_window(c: &Correction, width: f64, height: f64) -> Area {
    let tile = corrections::context_rect(c, [0.0, 0.0, width, height]);
    let reach = (tile[2].max(tile[3]) * 1.6 + tile[2].max(tile[3])) * 1.5 + 16.0;
    let (cx, cy) = (tile[0] + tile[2] / 2.0, tile[1] + tile[3] / 2.0);
    let (x0, y0) = ((cx - reach).max(0.0).floor(), (cy - reach).max(0.0).floor());
    let (x1, y1) = ((cx + reach).min(width).ceil(), (cy + reach).min(height).ceil());
    [x0, y0, x1 - x0, y1 - y0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg::Point;
    use oni_settings::id::Id;

    /// A textured frame whose neighbourhood of the dust is a copy of a block `known` away: the search
    /// must find that block, and the patch must carry its pixels, not the dust's.
    #[test]
    fn the_search_finds_the_place_the_surroundings_were_copied_from() {
        let (w, h) = (1600usize, 1200usize);
        // White noise smoothed over 17 pixels: no period, and a basin wider than the coarse step.
        let hash = |x: usize, y: usize| {
            let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
            v ^= v >> 13;
            v = v.wrapping_mul(1_274_126_177);
            (v >> 8) as f32 / 16_777_216.0
        };
        let noise: Vec<f32> = (0..w * h).map(|i| hash(i % w, i / w)).collect();
        let r = 8usize;
        let smooth: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let (mut sum, mut n) = (0.0, 0.0);
                for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                    for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                        sum += noise[yy * w + xx];
                        n += 1.0;
                    }
                }
                sum / n
            })
            .collect();
        // Stretched back to a photograph's contrast: a mean of 0.4 and a spread of 0.08.
        let mean = smooth.iter().sum::<f32>() / smooth.len() as f32;
        let spread = (smooth.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / smooth.len() as f32).sqrt();
        let smooth: Vec<f32> = smooth.iter().map(|v| 0.4 + 0.08 * (v - mean) / spread).collect();
        let texture = |x: usize, y: usize| smooth[y * w + x];
        let mut pixels: Vec<[f32; 4]> = (0..w * h).map(|i| { let v = smooth[i]; [v, v * 0.9, v * 0.8, 1.0] }).collect();
        let (dot_x, dot_y) = (800usize, 600usize);
        let known = (-288i64, 168i64);
        for y in dot_y - 200..dot_y + 200 {
            for x in dot_x - 200..dot_x + 200 {
                let from = ((y as i64 + known.1) as usize) * w + (x as i64 + known.0) as usize;
                pixels[y * w + x] = pixels[from];
            }
        }
        for y in dot_y - 20..=dot_y + 20 {
            for x in dot_x - 20..=dot_x + 20 {
                pixels[y * w + x] = [0.02, 0.02, 0.02, 1.0];
            }
        }
        let src = Region { x: 0, y: 0, width: w, height: h, pixels, frame_height: h as f64 };
        let c = Correction { id: Id::new(), stroke: vec![Point { x: dot_x as f64 / w as f64, y: dot_y as f64 / h as f64 }], radius: 30.0 };
        let (dx, dy) = best_offset(&src, &c, w as f64, h as f64);
        // The offset is y up: a block lower on the frame (y down + 168) is 168 under, −168 up.
        assert_eq!((dx, dy), (known.0 as f64, -known.1 as f64), "found {dx}, {dy}");
        let patch = generate(&src, &c, w as f64, h as f64);
        let (px, py) = (dot_x - patch.rect[0] as usize, dot_y - patch.rect[1] as usize);
        let healed = patch.pixels[py * patch.width + px][0];
        let want = texture((dot_x as i64 + known.0) as usize, (dot_y as i64 + known.1) as usize);
        assert!((healed - want).abs() < 0.02, "healed to {healed}, the copied block holds {want}");
        // Twin: the frame still holds the dust there, so a patch copied in place would be seen.
        assert!((src.pixels[dot_y * w + dot_x][0] - want).abs() > 0.2);
    }

    #[test]
    fn a_bias_lifts_the_copy_to_its_surroundings_tone() {
        // The right half a step brighter: a copy taken from the left half is lifted by the base.
        let (w, h) = (400usize, 300usize);
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| { let v = if i % w >= 200 { 0.5 } else { 0.4 }; [v, v, v, 1.0] }).collect();
        let src = Region { x: 0, y: 0, width: w, height: h, pixels, frame_height: h as f64 };
        let destination = Rect { x: 250.0, y: 100.0, w: 60.0, h: 60.0 };
        let source = Rect { x: 50.0, ..destination };
        let field = tone_field(&src, destination, source, Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
        assert!((field.base[0] - 0.1).abs() < 1e-4, "{:?}", field.base);
        // Twin: the same rings on one tone need no bias.
        let flat = tone_field(&src, destination, Rect { x: 320.0, ..destination }, Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
        assert!(flat.base[0].abs() < 1e-5);
    }
}
