//! `RGBRegistration`: where the red and blue passes lie on the green one, read on the log signal's
//! gradient, where the dye layers share their edges though not their contrast.

use oni_settings::frame::{Fit, Registration, RgbPasses};

/// `RGBRegistration.floor`: a cell's correlation peak under it says nothing.
pub const FLOOR: f64 = 0.3;
/// `shiftSide`: the preview's side, where a shift lands within 0.2 px of the full-resolution reading
/// once the roll's magnification is known.
pub const SHIFT_SIDE: f64 = 2000.0;
/// `magnificationFrames`: the frames of a roll its magnification is read on, at full resolution.
pub const MAGNIFICATION_FRAMES: usize = 3;

/// One band of a pass, rows top first.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
}

impl Plane {
    /// `plane(_:channel:)`: the pass's own channel alone; `None` for a pass of eight pixels a side or less.
    pub fn channel(width: usize, height: usize, pixels: &[[f32; 4]], channel: usize) -> Option<Plane> {
        (width > 8 && height > 8 && pixels.len() == width * height).then(|| Plane { width, height, values: pixels.iter().map(|p| p[channel]).collect() })
    }
}

/// A cell's centre and the shift that lays the pass on the green there, weighted by its peak squared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub weight: f64,
}

/// A pass against the green one: `k` its magnification's excess over 1, `theta` a small turn
/// (radians, in the plane's y-down frame), then a shift in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Field {
    pub k: f64,
    pub theta: f64,
    pub tx: f64,
    pub ty: f64,
}

/// The log signal's gradient magnitude over a square, so cells compare edges, not levels.
fn edges(p: &Plane, x0: i64, y0: i64, side: usize) -> Vec<f64> {
    let l = |x: i64, y: i64| f64::from(p.values[y as usize * p.width + x as usize]).max(1e-4).ln();
    let mut out = vec![0.0; side * side];
    for y in 0..side as i64 {
        for x in 0..side as i64 {
            let gx = l(x0 + x + 1, y0 + y) - l(x0 + x - 1, y0 + y);
            let gy = l(x0 + x, y0 + y + 1) - l(x0 + x, y0 + y - 1);
            out[y as usize * side + x as usize] = (gx * gx + gy * gy).sqrt();
        }
    }
    out
}

fn correlation(reference: &[f64], region: &[f64], stride: usize, ox: usize, oy: usize, side: usize) -> f64 {
    let n = (side * side) as f64;
    let (mut sa, mut sb, mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for y in 0..side {
        let row = (oy + y) * stride + ox;
        for x in 0..side {
            let (a, b) = (reference[y * side + x], region[row + x]);
            sa += a;
            sb += b;
            sab += a * b;
            saa += a * a;
            sbb += b * b;
        }
    }
    let spread = ((saa / n - sa * sa / n / n) * (sbb / n - sb * sb / n / n)).sqrt();
    if spread > 0.0 { (sab / n - sa / n * sb / n) / spread } else { 0.0 }
}

/// `cells`: a grid of cells over the frame's inner 84 %, each searched ±`reach` px then refined to a
/// fraction of a pixel; a cell whose peak sits on the search's rim or under `FLOOR` is dropped.
pub fn cells(green: &Plane, pass: &Plane, grid: usize, side: usize, reach: usize) -> Vec<Cell> {
    if (green.width, green.height) != (pass.width, pass.height) {
        return Vec::new();
    }
    let (w, h, reach_i, side_i) = (green.width as i64, green.height as i64, reach as i64, side as i64);
    let mut out = Vec::new();
    for j in 0..grid {
        for i in 0..grid {
            let cx = (green.width as f64 * (0.08 + 0.84 * (i as f64 + 0.5) / grid as f64)) as i64;
            let cy = (green.height as f64 * (0.08 + 0.84 * (j as f64 + 0.5) / grid as f64)) as i64;
            let (x0, y0) = (cx - side_i / 2, cy - side_i / 2);
            if !(x0 - reach_i > 1 && y0 - reach_i > 1 && x0 + side_i + reach_i < w - 1 && y0 + side_i + reach_i < h - 1) {
                continue;
            }
            let reference = edges(green, x0, y0, side);
            // The pass's edges over the cell and its whole search, once: each shift is then a window.
            let stride = side + 2 * reach;
            let region = edges(pass, x0 - reach_i, y0 - reach_i, stride);
            let span = 2 * reach + 1;
            let mut scores = vec![vec![-2.0; span]; span];
            let mut best = (0i64, 0i64, -2.0f64);
            for dy in -reach_i..=reach_i {
                for dx in -reach_i..=reach_i {
                    let score = correlation(&reference, &region, stride, (dx + reach_i) as usize, (dy + reach_i) as usize, side);
                    scores[(dy + reach_i) as usize][(dx + reach_i) as usize] = score;
                    if score > best.2 {
                        best = (dx, dy, score);
                    }
                }
            }
            let (bx, by) = (best.0 + reach_i, best.1 + reach_i);
            if !(best.2 > FLOOR && bx > 0 && bx < 2 * reach_i && by > 0 && by < 2 * reach_i) {
                continue;
            }
            let (bx, by) = (bx as usize, by as usize);
            let vertex = |a: f64, b: f64, c: f64| {
                let curve = a - 2.0 * b + c;
                if curve < 0.0 { 0.5 * (a - c) / curve } else { 0.0 }
            };
            out.push(Cell {
                x: cx as f64,
                y: cy as f64,
                dx: best.0 as f64 + vertex(scores[by][bx - 1], scores[by][bx], scores[by][bx + 1]),
                dy: best.1 as f64 + vertex(scores[by - 1][bx], scores[by][bx], scores[by + 1][bx]),
                weight: best.2 * best.2,
            });
        }
    }
    out
}

type Row = (Vec<f64>, f64, f64);

/// `fitted`: magnification, turn and shift by weighted least squares, then once more without the
/// cells past 2.5 times the median residual, so one cell on a layer's own detail cannot pull the fit.
pub fn fitted(cells: &[Cell], width: usize, height: usize) -> Option<Field> {
    let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
    let mut kept = cells.to_vec();
    let mut field = None;
    for round in 0..2 {
        if kept.len() < 4 {
            return field;
        }
        // dx = k·u − θ·v + tx, dy = k·v + θ·u + ty, with (u, v) the cell's place about the centre.
        let rows: Vec<Row> = kept
            .iter()
            .flat_map(|c| {
                let (u, v) = (c.x - cx, c.y - cy);
                [(vec![u, -v, 1.0, 0.0], c.dx, c.weight), (vec![v, u, 0.0, 1.0], c.dy, c.weight)]
            })
            .collect();
        let Some(s) = least_squares(&rows, 4) else { return field };
        let f = Field { k: s[0], theta: s[1], tx: s[2], ty: s[3] };
        field = Some(f);
        if round == 0 {
            kept = trimmed(&kept, |c| residual(c, &f, cx, cy));
        }
    }
    field
}

/// `shift`: turn and shift, the magnification given, which the roll's lens fixes once.
pub fn shift(cells: &[Cell], k: f64, width: usize, height: usize) -> Option<Field> {
    let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
    let mut kept = cells.to_vec();
    let mut field = None;
    for round in 0..2 {
        if kept.len() < 3 {
            return field;
        }
        let rows: Vec<Row> = kept
            .iter()
            .flat_map(|c| {
                let (u, v) = (c.x - cx, c.y - cy);
                [(vec![-v, 1.0, 0.0], c.dx - k * u, c.weight), (vec![u, 0.0, 1.0], c.dy - k * v, c.weight)]
            })
            .collect();
        let Some(s) = least_squares(&rows, 3) else { return field };
        let f = Field { k, theta: s[0], tx: s[1], ty: s[2] };
        field = Some(f);
        if round == 0 {
            kept = trimmed(&kept, |c| residual(c, &f, cx, cy));
        }
    }
    field
}

fn residual(c: &Cell, f: &Field, cx: f64, cy: f64) -> f64 {
    let (u, v) = (c.x - cx, c.y - cy);
    let ex = c.dx - (f.k * u - f.theta * v + f.tx);
    let ey = c.dy - (f.k * v + f.theta * u + f.ty);
    (ex * ex + ey * ey).sqrt()
}

fn trimmed(cells: &[Cell], residual: impl Fn(&Cell) -> f64) -> Vec<Cell> {
    let residuals: Vec<f64> = cells.iter().map(&residual).collect();
    let mut sorted = residuals.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    cells.iter().zip(residuals).filter(|(_, r)| *r <= (2.5 * median).max(0.05)).map(|(c, _)| *c).collect()
}

/// The normal equations solved by Gauss-Jordan on the largest pivot; `None` on a singular system.
fn least_squares(rows: &[Row], n: usize) -> Option<Vec<f64>> {
    let mut a = vec![vec![0.0; n + 1]; n];
    for (row, value, weight) in rows {
        for i in 0..n {
            for j in 0..n {
                a[i][j] += weight * row[i] * row[j];
            }
            a[i][n] += weight * row[i] * value;
        }
    }
    for column in 0..n {
        // `max(by:)` keeps the first of equal pivots.
        let pivot = (column..n).fold(column, |best, r| if a[best][column].abs() < a[r][column].abs() { r } else { best });
        if a[pivot][column].abs() <= 1e-12 {
            return None;
        }
        a.swap(column, pivot);
        for row in 0..n {
            if row == column {
                continue;
            }
            let factor = a[row][column] / a[column][column];
            for k in column..=n {
                a[row][k] -= factor * a[column][k];
            }
        }
    }
    Some((0..n).map(|i| a[i][n] / a[i][i]).collect())
}

/// `fit(_:width:)`: the fit laying a pass on the green one, from its measured field. The pass is read
/// at x + d(x), so the fit is d's inverse, turned and offset back into Core Image's y-up frame.
pub fn fit(field: &Field, width: usize) -> Fit {
    // x + d(x) = M (x − c) + c + t, with M = (1 + k)·I + θ·J: a scale ρ and a turn α.
    let rho = ((1.0 + field.k) * (1.0 + field.k) + field.theta * field.theta).sqrt();
    let alpha = field.theta.atan2(1.0 + field.k);
    // The inverse's offset, −M⁻¹ t, in the y-down frame, then y turned up.
    let (cosine, sine) = ((-alpha).cos() / rho, (-alpha).sin() / rho);
    let back = (-(cosine * field.tx - sine * field.ty), -(sine * field.tx + cosine * field.ty));
    Fit { scale: 1.0 / rho, rotation: alpha, dx: back.0 / width as f64, dy: -back.1 / width as f64 }
}

/// A frame's three bands, red, green then blue, decoded alone at a long side (`None`: full).
pub type Planes<'a> = &'a dyn Fn(&str, &RgbPasses, Option<f64>) -> Option<[Plane; 3]>;

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// `magnification(of:)`: each band's magnification on the green, a property of the lens, read at
/// full resolution on the roll's first frames and taken as their median.
pub fn magnification(frames: &[(String, RgbPasses)], planes: Planes) -> Option<(f64, f64)> {
    let (mut reds, mut blues) = (Vec::new(), Vec::new());
    for (lead, passes) in frames.iter().take(MAGNIFICATION_FRAMES) {
        let Some([red, green, blue]) = planes(lead, passes, None) else { continue };
        for (pass, into) in [(&red, &mut reds), (&blue, &mut blues)] {
            let found = cells(&green, pass, 7, 256, 6);
            if let Some(f) = fitted(&found, green.width, green.height) {
                into.push(f.k);
            }
        }
    }
    if reds.is_empty() || blues.is_empty() {
        return None;
    }
    Some((median(&mut reds), median(&mut blues)))
}

/// `registration(lead:passes:magnification:)`: each pass's turn and shift on the frame's preview.
pub fn registration(lead: &str, passes: &RgbPasses, magnification: (f64, f64), planes: Planes) -> Option<Registration> {
    let [red, green, blue] = planes(lead, passes, Some(SHIFT_SIDE))?;
    let fit_of = |pass: &Plane, k: f64| {
        let found = cells(&green, pass, 7, 128, 3);
        shift(&found, k, green.width, green.height).map(|f| fit(&f, green.width))
    };
    Some(Registration { red: fit_of(&red, magnification.0)?, blue: fit_of(&blue, magnification.1)? })
}

/// `Fit.transform(on:)` over a `w` × `h` frame: `[a, b, c, d, tx, ty]`, y up as Core Image counts.
pub fn transform(fit: &Fit, w: usize, h: usize) -> [f64; 6] {
    let (fw, fh) = (w as f64, h as f64);
    let (mx, my) = (fw / 2.0, fh / 2.0);
    let (cs, sn) = (fit.rotation.cos() * fit.scale, fit.rotation.sin() * fit.scale);
    let (a, b, c, d) = (cs, sn, -sn, cs);
    [a, b, c, d, mx - (a * mx + c * my) + fit.dx * fw, my - (b * mx + d * my) + fit.dy * fw]
}

pub fn inverted(t: &[f64; 6]) -> [f64; 6] {
    let [a, b, c, d, tx, ty] = *t;
    let det = a * d - b * c;
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    [ia, ib, ic, id, -(ia * tx + ic * ty), -(ib * tx + id * ty)]
}

/// `clampedToExtent().transformed(by:).cropped(to:)`: the plane carried by `t`, sampled bilinearly
/// at pixel centres with its edge held past it.
pub fn transformed(p: &Plane, t: &[f64; 6]) -> Plane {
    let [a, b, c, d, tx, ty] = inverted(t);
    let (w, h) = (p.width, p.height);
    let texel = |x: i64, y_up: i64| -> f32 {
        let col = x.clamp(0, w as i64 - 1) as usize;
        let row = (h as i64 - 1 - y_up).clamp(0, h as i64 - 1) as usize;
        p.values[row * w + col]
    };
    let mut values = Vec::with_capacity(w * h);
    for row in 0..h {
        let qy = h as f64 - row as f64 - 0.5;
        for col in 0..w {
            let qx = col as f64 + 0.5;
            let (ix, iy) = (a * qx + c * qy + tx - 0.5, b * qx + d * qy + ty - 0.5);
            let (x0, y0) = (ix.floor(), iy.floor());
            let (fx, fy) = ((ix - x0) as f32, (iy - y0) as f32);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let low = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1, y0) * fx;
            let up = texel(x0, y0 + 1) * (1.0 - fx) + texel(x0 + 1, y0 + 1) * fx;
            values.push(low * (1.0 - fy) + up * fy);
        }
    }
    Plane { width: w, height: h, values }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDE: usize = 900;

    /// Discs of many sizes and levels over a grey ground, edges in every direction, none on the grid
    /// the cells use: `RGBRegistration.selfCheck`'s textured frame.
    fn scene() -> Plane {
        let (w, h) = (SIDE, (SIDE as f64 * 0.7) as usize);
        let mut values = vec![0.3f32; w * h];
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..420 {
            let level = (0.05 + 0.9 * next()) as f32;
            let (x, y) = (next() * w as f64, next() * h as f64);
            let (r0, r1) = (3.0 + 14.0 * next(), 3.6 + 14.0 * next());
            let (inner, outer) = (r0.min(r1), r0.max(r1).max(r0.min(r1) + 0.6));
            let (lo_x, hi_x) = (((x - outer).floor().max(0.0)) as usize, ((x + outer).ceil() as usize).min(w));
            let (lo_y, hi_y) = (((y - outer).floor().max(0.0)) as usize, ((y + outer).ceil() as usize).min(h));
            for row in lo_y..hi_y {
                for col in lo_x..hi_x {
                    let r = ((col as f64 + 0.5 - x).powi(2) + (row as f64 + 0.5 - y).powi(2)).sqrt();
                    let alpha = ((outer - r) / (outer - inner)).clamp(0.0, 1.0) as f32;
                    let v = &mut values[row * w + col];
                    *v = level * alpha + *v * (1.0 - alpha);
                }
            }
        }
        Plane { width: w, height: h, values }
    }

    fn worst(cells: &[Cell]) -> f64 {
        if cells.is_empty() {
            return f64::INFINITY;
        }
        cells.iter().map(|c| c.dx.abs().max(c.dy.abs())).fold(0.0, f64::max)
    }

    /// `RGBRegistration.selfCheck`: a pass magnified and shifted by known amounts reads them back,
    /// laid back by the fit it lands on the green, and an untouched pass reads as identity.
    #[test]
    fn a_pass_s_magnification_turn_and_shift_read_back_off_its_edges() {
        let green = scene();
        let truth = Field { k: 6e-4, theta: 4e-4, tx: 0.7, ty: -0.45 };
        // The pass is read at x + d(x): built as the green laid by d's inverse.
        let frame = transform(&fit(&truth, SIDE), green.width, green.height);
        let pass = transformed(&green, &inverted(&frame));
        let found = cells(&green, &pass, 7, 96, 4);
        let field = fitted(&found, green.width, green.height).expect("a field read");
        let half = SIDE as f64 / 2.0;
        let error = ((field.k - truth.k).abs() * half).max((field.theta - truth.theta).abs() * half).max((field.tx - truth.tx).abs()).max((field.ty - truth.ty).abs());
        assert!(error < 0.08, "read back within {error:.3} px at the frame's edge over {} cells", found.len());
        let given = shift(&found, truth.k, green.width, green.height).expect("a shift read");
        let off = (given.tx - truth.tx).abs().max((given.ty - truth.ty).abs()).max((given.theta - truth.theta).abs() * half);
        assert!(off < 0.06, "the magnification given, turn and shift within {off:.3} px");
        // Laid back by the fit read, the pass lands on the green: the residual is the reading's, and
        // the two bilinear resamplings', which this frame's 49 textured cells show at their worst.
        let back = transformed(&pass, &transform(&fit(&field, SIDE), green.width, green.height));
        let backs = cells(&green, &back, 7, 96, 4);
        let mut offs: Vec<f64> = backs.iter().map(|c| c.dx.abs().max(c.dy.abs())).collect();
        offs.sort_by(f64::total_cmp);
        let (left, middle) = (worst(&backs), offs[offs.len() / 2]);
        assert!(backs.len() == 49 && left < 0.2 && middle < 0.06, "laid back, the pass lies within {left:.3} px of the green in every cell, {middle:.3} at the median");
        // Twin: laid the wrong way, by the fit's inverse, the pass lies twice as far off.
        let wrong = transformed(&pass, &inverted(&transform(&fit(&field, SIDE), green.width, green.height)));
        assert!(worst(&cells(&green, &wrong, 7, 96, 4)) > 1.0, "the fit's direction is what lays the pass");
        // Twin: an untouched pass reads as no magnification, no turn and no shift.
        let identity = fitted(&cells(&green, &green, 7, 96, 4), green.width, green.height).expect("identity read");
        assert!(identity.k.abs().max(identity.theta.abs()) * half < 0.02 && identity.tx.abs() < 0.02 && identity.ty.abs() < 0.02, "{identity:?}");
    }

    /// The roll's two readings through injected planes: the red pass carried, the blue untouched.
    #[test]
    fn a_roll_reads_its_magnification_once_and_each_frame_s_shift_on_its_own() {
        let green = scene();
        let truth = Field { k: 6e-4, theta: 0.0, tx: 0.7, ty: -0.45 };
        let red = transformed(&green, &inverted(&transform(&fit(&truth, SIDE), green.width, green.height)));
        let planes = |_: &str, _: &RgbPasses, _: Option<f64>| Some([red.clone(), green.clone(), green.clone()]);
        let passes = RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None };
        let frames = vec![("/roll/b".to_string(), passes.clone())];
        let (kr, kb) = magnification(&frames, &planes).expect("a magnification read");
        assert!((kr - truth.k).abs() * (SIDE as f64 / 2.0) < 0.08 && kb.abs() * (SIDE as f64 / 2.0) < 0.02, "red {kr:e}, blue {kb:e}");
        let r = registration("/roll/b", &passes, (kr, kb), &planes).expect("a registration read");
        let expected = fit(&truth, SIDE);
        assert!((r.red.dx - expected.dx).abs() * (SIDE as f64) < 0.08 && (r.red.dy - expected.dy).abs() * (SIDE as f64) < 0.08, "{:?} against {expected:?}", r.red);
        assert!(r.blue.dx.abs() * (SIDE as f64) < 0.02 && (r.blue.scale - 1.0).abs() < 1e-4, "{:?}", r.blue);
        // Twin: a frame whose passes cannot be read gives no registration, and no roll no magnification.
        let none = |_: &str, _: &RgbPasses, _: Option<f64>| None;
        assert!(registration("/roll/b", &passes, (kr, kb), &none).is_none() && magnification(&frames, &none).is_none());
    }

    #[test]
    fn the_normal_equations_solve_and_a_singular_system_says_so() {
        // 2x + y = 5, x − y = 1: x = 2, y = 1.
        let rows: Vec<Row> = vec![(vec![2.0, 1.0], 5.0, 1.0), (vec![1.0, -1.0], 1.0, 1.0)];
        let s = least_squares(&rows, 2).unwrap();
        assert!((s[0] - 2.0).abs() < 1e-12 && (s[1] - 1.0).abs() < 1e-12, "{s:?}");
        // Twin: one equation twice leaves the system singular.
        assert!(least_squares(&[(vec![1.0, 1.0], 2.0, 1.0), (vec![2.0, 2.0], 4.0, 1.0)], 2).is_none());
    }
}
