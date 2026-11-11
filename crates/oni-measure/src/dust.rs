//! Auto De-dust's reading: how far inside its window's dense end each pixel of a framed frame lies, and the
//! dust its shapes show. Read once on a click, it answers every sensitivity with no pixel read again, and writes nothing.

use oni_pipeline::conversion::rendered_ceiling;
use oni_settings::PipelineSettings;
use oni_settings::axis::{density_axis, pipeline};
use std::collections::HashMap;

/// The largest speck, a long hair; a highlight past the window fills more.
pub const MAX_AREA: u32 = 8192;
/// How far off the frame's edge a speck must stand, in pixels: the holder and the rebate lie there.
pub const EDGE: u32 = 16;
/// Where a speck's edge is read, in pixels off it: dust lies on the film, in the scan's focus.
pub const JUMP_AT: i64 = 2;
/// The least share of its jump a speck's edge makes within `JUMP_AT`: dust makes 0.46 to 1 at 47 Mpx, the
/// image's glints and lamps, softened by the camera's lens and the film, 0.26 to 0.39 at 26 Mpx.
pub const SHARP: f32 = 0.45;
/// How far inside the window a reading goes at full sensitivity, in density.
const REACH: f32 = 0.8;
/// The insets shapes are read at: a shape past one can merge with a highlight past the next, so each is
/// read and a speck stays found once found.
const LEVELS: [f32; 5] = [0.0, 0.2, 0.4, 0.6, 0.8];
/// The jump over its ground the least channel of a speck must make at the least sensitive and at the most:
/// grain in a highlight rises 0.09 at most, a speck 0.11 to 0.73.
const JUMPS: (f32, f32) = (0.2, 0.1);
/// A map's texel for what no sensitivity shows.
pub const NEVER: u8 = 255;
/// The last of the slider's notches a map's texels tell apart.
const NOTCHES: f32 = 254.0;

mod speck;
mod stroke;
pub use speck::speck;
pub use stroke::stroke;

/// The slider's position as a notch: a texel shows at its own notch and every one above.
pub fn notch(sensitivity: f32) -> u32 {
    (sensitivity.clamp(0.0, 1.0) * NOTCHES).floor() as u32
}

/// The first notch at which what shows from `sensitivity` is shown.
fn texel(sensitivity: f32) -> u8 {
    (sensitivity.clamp(0.0, 1.0) * NOTCHES).ceil() as u8
}

/// The window's dense end as the light a framed pixel holds, per channel: the white of a negative, the
/// black of a positive. Past it, stage 4's place on the window leaves 0…1, with no logarithm per pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub densest: [f32; 3],
    pub opaque_below: [f32; 3],
    pub gain: [f32; 3],
    pub pedestal: f32,
    pub guard: f32,
}

impl Limits {
    /// The roll's window and gains when the frame is on one, else the frame's own: a hand's later move on
    /// this frame's levels is a look, and leaves what its film holds where the roll put it.
    pub fn of(s: &PipelineSettings) -> Limits {
        let on = s.roll_reference.as_ref().map(|r| oni_core::balance::written(r, s));
        let on = on.as_ref().unwrap_or(s);
        let ceiling = rendered_ceiling(on);
        let (pedestal, guard) = (density_axis::pedestal(ceiling), density_axis::log_guard(ceiling));
        let windows = [on.levels.red, on.levels.green, on.levels.blue];
        let inverted = on.mode.invert_flag() > 0.5;
        let densest = [0, 1, 2].map(|c| if inverted { windows[c].white } else { pipeline::DMAX - windows[c].black });
        Limits { densest, opaque_below: [f32::NEG_INFINITY; 3], gain: on.gains.stops.map(f32::exp2), pedestal, guard }.closed_in(0.0)
    }

    /// The dense end read `by` density inside the window; under the guard the kernel reads the guard, so that end is never passed.
    pub fn closed_in(self, by: f32) -> Limits {
        let opaque_below = [0, 1, 2].map(|c| {
            let light = 10f32.powf(by - self.densest[c]);
            if light > self.guard { light / self.gain[c] - self.pedestal } else { f32::NEG_INFINITY }
        });
        Limits { opaque_below, ..self }
    }

    /// A channel's density as stage 4 reads it.
    pub fn density(&self, p: &[f32; 4], c: usize) -> f32 {
        -((p[c] + self.pedestal) * self.gain[c]).max(self.guard).log10()
    }

    /// Whether a channel of `p` is past the dense end.
    pub fn past(&self, p: &[f32; 4]) -> bool {
        (0..3).any(|c| p[c] < self.opaque_below[c])
    }

    /// The sensitivity `p` shows from: 0 past the window, 1 at the reach; only a channel under `deep` is read.
    fn shows_from(&self, p: &[f32; 4], deep: &Limits) -> f32 {
        (0..3).filter(|&c| p[c] < deep.opaque_below[c]).map(|c| ((self.densest[c] - self.density(p, c)) / REACH).max(0.0)).fold(f32::INFINITY, f32::min)
    }
}

/// A row's pixels side by side: columns `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub row: u32,
    pub start: u32,
    pub end: u32,
}

/// One connected shape of pixels past the window, 8-connected.
#[derive(Clone, Debug, PartialEq)]
pub struct Speck {
    pub area: u32,
    pub runs: Vec<Run>,
    /// Left, top, right and bottom, the last two past the shape.
    pub bounds: [u32; 4],
}

/// A shape a reading can write: the sensitivity it shows from, and the one where a shape holding it takes over.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    speck: Speck,
    from: f32,
    until: f32,
}

/// What each pixel shows from, reduced so that nothing fades, for the display's kernel: a texel holds the
/// first notch at which its pixels are past the window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    pub width: u32,
    pub height: u32,
    pub texels: Vec<u8>,
}

/// A frame read once for every sensitivity.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub width: usize,
    pub height: usize,
    pub map: Map,
    /// The pixels past the window itself.
    pub past: u64,
    candidates: Vec<Candidate>,
}

/// Every row's pixels past `limits` as runs, top first, read on all cores.
fn runs_of(pixels: &[[f32; 4]], width: usize, limits: &Limits) -> Vec<Run> {
    let height = pixels.len() / width.max(1);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = height.div_ceil(threads).max(1);
    let parts: Vec<Vec<Run>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..height.div_ceil(per))
            .map(|part| {
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for row in part * per..((part + 1) * per).min(height) {
                        let mut open: Option<u32> = None;
                        for (x, p) in pixels[row * width..(row + 1) * width].iter().enumerate() {
                            match (open, limits.past(p)) {
                                (None, true) => open = Some(x as u32),
                                (Some(start), false) => {
                                    out.push(Run { row: row as u32, start, end: x as u32 });
                                    open = None;
                                }
                                _ => {}
                            }
                        }
                        if let Some(start) = open {
                            out.push(Run { row: row as u32, start, end: width as u32 });
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a row reader")).collect()
    });
    parts.concat()
}

fn root(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        parent[i as usize] = parent[parent[i as usize] as usize];
        i = parent[i as usize];
    }
    i
}

/// The runs joined into 8-connected shapes.
fn gathered(runs: &[Run]) -> Vec<Speck> {
    let mut parent: Vec<u32> = (0..runs.len() as u32).collect();
    // The runs above overlap a run's columns, a pixel wider each way for the diagonals.
    let (mut above, mut here) = (0usize, 0usize);
    for i in 0..runs.len() {
        let run = runs[i];
        if i > 0 && runs[i - 1].row != run.row {
            above = here;
            here = i;
        }
        if run.row == 0 {
            continue;
        }
        while above < here && runs[above].row + 1 == run.row && runs[above].end < run.start {
            above += 1;
        }
        let mut k = above;
        while k < here && runs[k].start <= run.end {
            if runs[k].row + 1 == run.row && runs[k].end >= run.start {
                let (a, b) = (root(&mut parent, k as u32), root(&mut parent, i as u32));
                parent[a.max(b) as usize] = a.min(b);
            }
            k += 1;
        }
    }
    let mut at = HashMap::new();
    let mut out: Vec<Speck> = Vec::new();
    for (i, &run) in runs.iter().enumerate() {
        let r = root(&mut parent, i as u32);
        let index = *at.entry(r).or_insert_with(|| {
            out.push(Speck { area: 0, runs: Vec::new(), bounds: [run.start, run.row, run.end, run.row + 1] });
            out.len() - 1
        });
        let s = &mut out[index];
        s.area += run.end - run.start;
        s.runs.push(run);
        s.bounds = [s.bounds[0].min(run.start), s.bounds[1].min(run.row), s.bounds[2].max(run.end), s.bounds[3].max(run.row + 1)];
    }
    out
}

/// Every shape past `limits`, before any rule: what the veil shows.
pub fn shapes(pixels: &[[f32; 4]], width: usize, limits: &Limits) -> Vec<Speck> {
    gathered(&runs_of(pixels, width, limits))
}

/// The sensitivity a shape read at `level` with this jump shows from: its level reached, its jump asked no more.
fn shows_from(level: usize, jump: f32) -> f32 {
    (level as f32 / (LEVELS.len() - 1) as f32).max(((JUMPS.0 - jump) / (JUMPS.0 - JUMPS.1)).max(0.0))
}

fn candidates(pixels: &[[f32; 4]], width: usize, limits: &Limits, level: usize) -> Vec<Candidate> {
    let shown: Vec<Speck> = gathered(&runs_of(pixels, width, &limits.closed_in(LEVELS[level]))).into_iter().filter(|s| s.area <= MAX_AREA).collect();
    // Shapes crowd where a sky nears the window: small parts, read on all cores, keep their order.
    let per = shown.len().div_ceil(4 * std::thread::available_parallelism().map_or(1, |n| n.get())).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = shown
            .chunks(per)
            .map(|part| scope.spawn(move || part.iter().filter_map(|s| speck(s, pixels, width, limits)).map(|(speck, jump)| Candidate { speck, from: shows_from(level, jump), until: f32::INFINITY }).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a speck reader")).collect()
    })
}

/// Whether a shape's first pixel lies in another's, as a shape read at one inset lies in what the next reads.
fn lies_in(inner: &Speck, outer: &Speck) -> bool {
    let Some(first) = inner.runs.first() else { return false };
    outer.runs.iter().any(|r| r.row == first.row && r.start <= first.start && first.start < r.end)
}

/// The map's size for a frame and the scale that reaches it.
fn reduced(width: usize, height: usize, side: usize) -> (u32, u32, f64) {
    let scale = (side as f64 / width.max(height).max(1) as f64).min(1.0);
    let cells = |v: usize| ((v as f64 * scale).ceil() as u32).max(1);
    (cells(width), cells(height), scale)
}

/// Each map row's earliest notch among the pixels it holds, read on all cores, and the pixels past the window itself.
fn past_notches(pixels: &[[f32; 4]], width: usize, limits: &Limits, side: usize) -> (Map, u64) {
    let height = pixels.len() / width.max(1);
    let (w, h, scale) = reduced(width, height, side);
    let deep = limits.closed_in(REACH);
    let mut texels = vec![NEVER; w as usize * h as usize];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = (h as usize).div_ceil(threads).max(1);
    let past: u64 = std::thread::scope(|scope| {
        let handles: Vec<_> = texels
            .chunks_mut(w as usize * per)
            .enumerate()
            .map(|(part, cells)| {
                let deep = &deep;
                scope.spawn(move || {
                    let mut past = 0u64;
                    let first = part * per;
                    // A pixel belongs to the cell its top-left corner falls in.
                    let row_of = |cell: usize| ((cell as f64 / scale).ceil() as usize).min(height);
                    for (offset, line) in cells.chunks_mut(w as usize).enumerate() {
                        let cell = first + offset;
                        for row in row_of(cell)..row_of(cell + 1) {
                            for (x, p) in pixels[row * width..(row + 1) * width].iter().enumerate() {
                                let s = limits.shows_from(p, deep);
                                if s.is_finite() {
                                    let t = &mut line[((x as f64 * scale) as usize).min(w as usize - 1)];
                                    *t = (*t).min(texel(s));
                                    past += u64::from(s == 0.0);
                                }
                            }
                        }
                    }
                    past
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a map row")).sum()
    });
    (Map { width: w, height: h, texels }, past)
}

/// A framed frame read for every sensitivity, on a map of long side `side`; `pixels` hold its rows top first.
pub fn read(pixels: &[[f32; 4]], width: usize, limits: &Limits, side: usize) -> Reading {
    let height = pixels.len() / width.max(1);
    let (map, past) = past_notches(pixels, width, limits, side);
    let levels: Vec<Vec<Candidate>> = (0..LEVELS.len()).map(|level| candidates(pixels, width, limits, level)).collect();
    let mut found: Vec<Candidate> = levels.concat();
    let held: Vec<f32> = found
        .iter()
        .enumerate()
        .map(|(i, c)| {
            // The same pixels read at two insets: the deeper, later in the list, takes over.
            let holds = |j: usize, o: &Candidate| (o.speck.area, j) > (c.speck.area, i) && lies_in(&c.speck, &o.speck);
            found.iter().enumerate().filter(|(j, o)| holds(*j, o)).map(|(_, o)| o.from).fold(f32::INFINITY, f32::min)
        })
        .collect();
    for (c, until) in found.iter_mut().zip(held) {
        c.until = until;
    }
    Reading { width, height, map, past, candidates: found }
}

impl Reading {
    /// The dust at `sensitivity`, on the slider's own notches: everything found below it stays, a speck read at two insets counting once, whole.
    pub fn at(&self, sensitivity: f32) -> Vec<&Speck> {
        let n = notch(sensitivity);
        self.candidates.iter().filter(|c| u32::from(texel(c.from)) <= n && (c.until > 1.0 || u32::from(texel(c.until)) > n)).map(|c| &c.speck).collect()
    }
}

#[cfg(test)]
mod tests;
