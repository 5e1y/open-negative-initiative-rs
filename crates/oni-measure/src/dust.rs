//! Auto De-dust's reading: the pixels of a framed frame past the window its conversion reads, gathered
//! into shapes, and the shapes that are dust. It runs on a click and writes nothing: the host lays what it shows.

use oni_pipeline::conversion::rendered_ceiling;
use oni_settings::PipelineSettings;
use oni_settings::axis::{density_axis, pipeline};
use std::collections::HashMap;

/// The smallest shape read as dust: a pixel or two past the window is the sensor's noise.
pub const MIN_AREA: u32 = 3;
/// The largest, a long hair; a highlight past the window fills more.
pub const MAX_AREA: u32 = 8192;
/// How far off the frame's edge a shape must stand, in pixels: the holder and the rebate lie there.
pub const EDGE: u32 = 16;
/// Where a shape's jump is read, in pixels off it: dust lies on the film, in the scan's focus.
pub const JUMP_AT: i64 = 2;
/// The density the least channel must jump there: the image's glints measure 0.05 to 0.17 on rolls #8 and Lumix, dust 0.34.
pub const MIN_JUMP: f32 = 0.2;
/// The least channel's jump over the most's: dust stops every colour alike, where a glint's blue jumps up to five times its red.
pub const NEUTRAL: f32 = 0.6;

mod stroke;
pub use stroke::stroke;

/// Which end of the window a pixel is past: past the film's densest, or lighter than its clearest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Denser than the window: dust or a hair stopping the light.
    Opaque,
    /// Clearer than the window: a scratch through the emulsion.
    Clear,
}

/// The window as the light a framed pixel holds, per channel: past either limit, stage 4's place on the
/// window leaves 0…1, with no logarithm per pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub opaque_below: [f32; 3],
    pub clear_above: [f32; 3],
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
        let gain = on.gains.stops.map(f32::exp2);
        let mut limits = Limits { opaque_below: [f32::NEG_INFINITY; 3], clear_above: [f32::INFINITY; 3], gain, pedestal, guard };
        for c in 0..3 {
            // A negative's window reads the density, a positive's its complement on the axis.
            let (densest, clearest) = if inverted {
                (windows[c].white, windows[c].black)
            } else {
                (pipeline::DMAX - windows[c].black, pipeline::DMAX - windows[c].white)
            };
            let light = |density: f32| 10f32.powf(-density);
            // Under the guard the kernel reads the guard: the window's end is never passed.
            if light(densest) > guard {
                limits.opaque_below[c] = light(densest) / gain[c] - pedestal;
            }
            limits.clear_above[c] = light(clearest) / gain[c] - pedestal;
        }
        limits
    }

    /// A channel's density as stage 4 reads it.
    pub fn density(&self, p: &[f32; 4], c: usize) -> f32 {
        -((p[c] + self.pedestal) * self.gain[c]).max(self.guard).log10()
    }

    /// The side `p` is past, opaque first: a channel past either limit is past the window.
    pub fn past(&self, p: &[f32; 4]) -> Option<Side> {
        if (0..3).any(|c| p[c] < self.opaque_below[c]) {
            Some(Side::Opaque)
        } else if (0..3).any(|c| p[c] > self.clear_above[c]) {
            Some(Side::Clear)
        } else {
            None
        }
    }
}

/// A row's pixels side by side: columns `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub row: u32,
    pub start: u32,
    pub end: u32,
}

/// One connected shape of pixels past the window, 8-connected, on one side.
#[derive(Clone, Debug, PartialEq)]
pub struct Speck {
    pub side: Side,
    pub area: u32,
    pub runs: Vec<Run>,
    /// Left, top, right and bottom, the last two past the shape.
    pub bounds: [u32; 4],
}

/// What a frame holds past its window, and which of those shapes are dust; rows top first.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub width: usize,
    pub height: usize,
    pub past: Vec<Run>,
    pub dust: Vec<Speck>,
}

/// Every row's pixels past the window as runs, top first, read on all cores.
fn runs_of(pixels: &[[f32; 4]], width: usize, limits: &Limits) -> Vec<(Run, Side)> {
    let height = pixels.len() / width.max(1);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = height.div_ceil(threads).max(1);
    let parts: Vec<Vec<(Run, Side)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..height.div_ceil(per))
            .map(|part| {
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for row in part * per..((part + 1) * per).min(height) {
                        let mut open: Option<(u32, Side)> = None;
                        for (x, p) in pixels[row * width..(row + 1) * width].iter().enumerate() {
                            let now = limits.past(p);
                            if let Some((start, was)) = open
                                && now != Some(was)
                            {
                                out.push((Run { row: row as u32, start, end: x as u32 }, was));
                                open = None;
                            }
                            if open.is_none() {
                                open = now.map(|s| (x as u32, s));
                            }
                        }
                        if let Some((start, was)) = open {
                            out.push((Run { row: row as u32, start, end: width as u32 }, was));
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

/// The runs joined into 8-connected shapes of one side.
fn gathered(runs: &[(Run, Side)]) -> Vec<Speck> {
    let mut parent: Vec<u32> = (0..runs.len() as u32).collect();
    // The runs above overlap a run's columns, a pixel wider each way for the diagonals.
    let (mut above, mut here) = (0usize, 0usize);
    for i in 0..runs.len() {
        let (run, side) = runs[i];
        if i > 0 && runs[i - 1].0.row != run.row {
            above = here;
            here = i;
        }
        if run.row == 0 {
            continue;
        }
        while above < here && runs[above].0.row + 1 == run.row && runs[above].0.end < run.start {
            above += 1;
        }
        let mut k = above;
        while k < here && runs[k].0.start <= run.end {
            if runs[k].0.row + 1 == run.row && runs[k].1 == side && runs[k].0.end >= run.start {
                let (a, b) = (root(&mut parent, k as u32), root(&mut parent, i as u32));
                parent[a.max(b) as usize] = a.min(b);
            }
            k += 1;
        }
    }
    let mut at = HashMap::new();
    let mut out: Vec<Speck> = Vec::new();
    for i in 0..runs.len() {
        let (run, side) = runs[i];
        let r = root(&mut parent, i as u32);
        let index = *at.entry(r).or_insert_with(|| {
            out.push(Speck { side, area: 0, runs: Vec::new(), bounds: [run.start, run.row, run.end, run.row + 1] });
            out.len() - 1
        });
        let s = &mut out[index];
        s.area += run.end - run.start;
        s.runs.push(run);
        s.bounds = [s.bounds[0].min(run.start), s.bounds[1].min(run.row), s.bounds[2].max(run.end), s.bounds[3].max(run.row + 1)];
    }
    out
}

/// Each channel's jump across a shape's edge towards its side: the median between its pixels and the pixels
/// inside the window `off` away, so a highlight's far denser core, with nothing near to pair it, counts for nothing.
pub fn jumps(s: &Speck, pixels: &[[f32; 4]], width: usize, limits: &Limits, off: i64) -> [f32; 3] {
    let height = pixels.len() / width.max(1);
    let (w, h) = (width as i64, height as i64);
    let mut across: [Vec<f32>; 3] = Default::default();
    for r in &s.runs {
        for x in r.start..r.end {
            let p = &pixels[r.row as usize * width + x as usize];
            for (dx, dy) in [(off, 0), (-off, 0), (0, off), (0, -off), (off, off), (-off, off), (off, -off), (-off, -off)] {
                let (nx, ny) = (i64::from(x) + dx, i64::from(r.row) + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let q = &pixels[ny as usize * width + nx as usize];
                if limits.past(q).is_some() {
                    continue;
                }
                for (c, list) in across.iter_mut().enumerate() {
                    list.push(limits.density(p, c) - limits.density(q, c));
                }
            }
        }
    }
    [0, 1, 2].map(|c| {
        let list = &mut across[c];
        if list.is_empty() {
            return 0.0;
        }
        list.sort_by(|a, b| a.total_cmp(b));
        let jump = list[list.len() / 2];
        if s.side == Side::Opaque { jump } else { -jump }
    })
}

/// Whether a shape's jumps are what a speck on the film makes: sharp and alike in every channel.
pub fn dust_like(j: [f32; 3]) -> bool {
    let (least, most) = (j[0].min(j[1]).min(j[2]), j[0].max(j[1]).max(j[2]));
    least >= MIN_JUMP && least >= NEUTRAL * most
}

fn is_dust(s: &Speck, pixels: &[[f32; 4]], width: usize, height: usize, limits: &Limits) -> bool {
    let inside = s.bounds[0] >= EDGE && s.bounds[1] >= EDGE && s.bounds[2] + EDGE <= width as u32 && s.bounds[3] + EDGE <= height as u32;
    inside && (MIN_AREA..=MAX_AREA).contains(&s.area) && dust_like(jumps(s, pixels, width, limits, JUMP_AT))
}

/// What a framed frame holds past `limits`; `pixels` hold its rows top first.
pub fn read(pixels: &[[f32; 4]], width: usize, limits: &Limits) -> Found {
    let height = pixels.len() / width.max(1);
    let runs = runs_of(pixels, width, limits);
    let dust = gathered(&runs).into_iter().filter(|s| is_dust(s, pixels, width, height, limits)).collect();
    Found { width, height, past: runs.into_iter().map(|(run, _)| run).collect(), dust }
}

#[cfg(test)]
mod tests;
