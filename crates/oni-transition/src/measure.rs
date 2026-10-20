//! Apple's decode against LibRaw's on one frame: aligned to the pixel, the squares both decoders read
//! clearly paired, then the camera's 3x3 from LibRaw's Rec. 2020 to Apple's fitted over those pairs.

/// The side of the squares both decodes are averaged over, as `scripts/decoder-gap` averages them.
pub const BLOCK: usize = 8;
/// A square counts when every channel lies inside this band in both decodes, off floor and clip.
pub const USABLE: (f32, f32) = (2e-4, 0.9);
/// The offsets searched around the frames' size difference, in pixels.
pub const REACH: isize = 8;
/// The fewest squares of the frame that must count, or the frame tells too little.
pub const MIN_SHARE: f64 = 0.2;
/// Below this the two decodes do not show the same picture at the offset found.
pub const MIN_CORRELATION: f64 = 0.98;
/// The textured squares the alignment is searched on, the frame's sharpest edges.
const SAMPLE: usize = 3000;

/// Linear Rec. 2020 pixels, row-major RGBA.
#[derive(Clone, Copy)]
pub struct Plane<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a [[f32; 4]],
}

/// A decode averaged over `BLOCK`-pixel squares laid from its top-left pixel.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub means: Vec<[f32; 3]>,
}

impl Grid {
    pub fn of(plane: Plane<'_>) -> Grid {
        let (width, height) = (plane.width / BLOCK, plane.height / BLOCK);
        let means = (0..width * height).map(|i| block(plane, (i % width) * BLOCK, (i / width) * BLOCK)).collect();
        Grid { width, height, means }
    }
}

pub type Rgb = [f64; 3];
pub type M3 = [[f64; 3]; 3];

/// One frame's measure: its squares paired, Apple over LibRaw per channel, and what they rest on.
#[derive(Debug, Clone, PartialEq)]
pub struct Shift {
    /// Each clear square, Apple's then LibRaw's.
    pub pairs: Vec<(Rgb, Rgb)>,
    pub stops: [f64; 3],
    /// Where Apple's top-left pixel falls in LibRaw's frame.
    pub offset: (isize, isize),
    pub correlation: f64,
    pub share: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Too few clear, textured squares to align on.
    Featureless,
    Misaligned(f64),
    TooFewUsable(f64),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Featureless => f.write_str("the frame is too flat to compare the two decoders"),
            Refusal::Misaligned(c) => write!(f, "the two decoders do not show the same picture (correlation {c:.3})"),
            Refusal::TooFewUsable(s) => write!(f, "too little of the frame is neither clipped nor black ({:.0} %)", s * 100.0),
        }
    }
}

fn block(plane: Plane<'_>, x0: usize, y0: usize) -> [f32; 3] {
    let mut s = [0f64; 3];
    for y in y0..y0 + BLOCK {
        for p in &plane.pixels[y * plane.width + x0..y * plane.width + x0 + BLOCK] {
            (0..3).for_each(|c| s[c] += f64::from(p[c]));
        }
    }
    s.map(|v| (v / (BLOCK * BLOCK) as f64) as f32)
}

fn usable(v: &[f32; 3]) -> bool {
    v.iter().all(|c| *c > USABLE.0 && *c < USABLE.1)
}

fn luminance(v: &[f32; 3]) -> f64 {
    (f64::from(v[0]) + 2.0 * f64::from(v[1]) + f64::from(v[2])).max(1e-9).log2()
}

/// LibRaw's square under Apple's square `(bx, by)` at `offset`, when it lies inside LibRaw's frame.
fn under(libraw: Plane<'_>, bx: usize, by: usize, offset: (isize, isize)) -> Option<[f32; 3]> {
    let (x, y) = ((bx * BLOCK) as isize + offset.0, (by * BLOCK) as isize + offset.1);
    let fits = x >= 0 && y >= 0 && x as usize + BLOCK <= libraw.width && y as usize + BLOCK <= libraw.height;
    fits.then(|| block(libraw, x as usize, y as usize))
}

fn correlation(pairs: &[(f64, f64)]) -> f64 {
    let n = pairs.len() as f64;
    let (ma, mb) = pairs.iter().fold((0.0, 0.0), |(a, b), p| (a + p.0 / n, b + p.1 / n));
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (a, b) in pairs {
        ab += (a - ma) * (b - mb);
        aa += (a - ma) * (a - ma);
        bb += (b - mb) * (b - mb);
    }
    if aa <= 0.0 || bb <= 0.0 { 0.0 } else { ab / (aa * bb).sqrt() }
}

pub fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    match v.len() {
        0 => 0.0,
        n if n % 2 == 1 => v[n / 2],
        n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
    }
}

/// The interior squares, edges left out: a decoder darkens or rebuilds its outer rows.
fn interior(grid: &Grid) -> impl Iterator<Item = (usize, usize)> + '_ {
    (1..grid.height.saturating_sub(1)).flat_map(move |y| (1..grid.width.saturating_sub(1)).map(move |x| (x, y)))
}

/// The offset at which LibRaw's squares best follow Apple's on the frame's sharpest squares.
fn aligned(apple: &Grid, libraw: Plane<'_>) -> Result<((isize, isize), f64), Refusal> {
    let at = |x: usize, y: usize| &apple.means[y * apple.width + x];
    let mut textured: Vec<(f64, usize, usize)> = interior(apple)
        .filter(|&(x, y)| usable(at(x, y)) && usable(at(x + 1, y)) && usable(at(x, y + 1)))
        .map(|(x, y)| {
            let l = luminance(at(x, y));
            ((l - luminance(at(x + 1, y))).abs() + (l - luminance(at(x, y + 1))).abs(), x, y)
        })
        .collect();
    if textured.len() < 200 {
        return Err(Refusal::Featureless);
    }
    textured.sort_by(|a, b| b.0.total_cmp(&a.0));
    textured.truncate(SAMPLE);
    let span = |l: usize, a: usize| {
        let d = l as isize - (a * BLOCK) as isize;
        (d.min(0) - REACH)..=(d.max(0) + REACH)
    };
    let mut best = ((0, 0), f64::MIN);
    for dy in span(libraw.height, apple.height) {
        for dx in span(libraw.width, apple.width) {
            let pairs: Vec<(f64, f64)> = textured
                .iter()
                .filter_map(|&(_, x, y)| under(libraw, x, y, (dx, dy)).map(|l| (luminance(at(x, y)), luminance(&l))))
                .collect();
            if pairs.len() * 2 < textured.len() {
                continue;
            }
            let r = correlation(&pairs);
            if r > best.1 {
                best = ((dx, dy), r);
            }
        }
    }
    if best.1 < MIN_CORRELATION {
        return Err(Refusal::Misaligned(best.1.max(0.0)));
    }
    Ok(best)
}

/// Apple over LibRaw on one frame: its clear squares paired, and each channel's median ratio in stops.
pub fn fit(apple: &Grid, libraw: Plane<'_>) -> Result<Shift, Refusal> {
    let (offset, correlation) = aligned(apple, libraw)?;
    let mut pairs = Vec::new();
    let mut total = 0usize;
    for (x, y) in interior(apple) {
        total += 1;
        let a = &apple.means[y * apple.width + x];
        let Some(l) = under(libraw, x, y, offset) else { continue };
        if usable(a) && usable(&l) {
            pairs.push((a.map(f64::from), l.map(f64::from)));
        }
    }
    let share = pairs.len() as f64 / total.max(1) as f64;
    if share < MIN_SHARE {
        return Err(Refusal::TooFewUsable(share));
    }
    let stops = [0, 1, 2].map(|c| median(pairs.iter().map(|(a, l)| (a[c] / l[c]).log2()).collect()));
    Ok(Shift { pairs, stops, offset, correlation, share })
}

/// The camera's shift from its frames' measures: each channel's median.
pub fn combined(shifts: &[[f64; 3]]) -> [f64; 3] {
    [0, 1, 2].map(|c| median(shifts.iter().map(|s| s[c]).collect()))
}

fn solve(a: M3, b: Rgb) -> Option<Rgb> {
    let det = |m: M3| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    (d.abs() > 1e-12).then(|| {
        [0, 1, 2].map(|k| {
            let mut m = a;
            (0..3).for_each(|r| m[r][k] = b[r]);
            det(m) / d
        })
    })
}

/// LibRaw to Apple, each row minimising the relative error `(M l)_c / a_c - 1` over every pair:
/// a density error, so the negative's dense and thin squares weigh alike.
pub fn matrix(pairs: &[(Rgb, Rgb)]) -> Option<M3> {
    let rows: Vec<Rgb> = (0..3)
        .map(|c| {
            let (mut ata, mut atb) = ([[0f64; 3]; 3], [0f64; 3]);
            for (a, l) in pairs {
                let row = l.map(|v| v / a[c]);
                for i in 0..3 {
                    atb[i] += row[i];
                    (0..3).for_each(|j| ata[i][j] += row[i] * row[j]);
                }
            }
            solve(ata, atb)
        })
        .collect::<Option<_>>()?;
    Some([rows[0], rows[1], rows[2]])
}

pub fn apply(m: &M3, v: Rgb) -> Rgb {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

/// Root mean square of the density error per channel, Apple against `m` times LibRaw (log10).
pub fn residual(pairs: &[(Rgb, Rgb)], m: &M3) -> Rgb {
    let n = pairs.len().max(1) as f64;
    let mut sq = [0f64; 3];
    for (a, l) in pairs {
        let fitted = apply(m, *l);
        (0..3).for_each(|c| sq[c] += (a[c].log10() - fitted[c].max(1e-9).log10()).powi(2) / n);
    }
    sq.map(f64::sqrt)
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
