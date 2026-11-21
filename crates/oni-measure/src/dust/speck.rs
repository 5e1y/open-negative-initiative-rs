//! A shape past the window judged whole: the speck it shows, read against the ground around it.

use super::{EDGE, JUMP_AT, JUMPS, Limits, MAX_AREA, Run, SHARP, Speck};
use std::collections::HashSet;

/// Where a speck's ground is read, in pixels off what shows it: past the scan's blur and a flake's body.
const GROUND_AT: u32 = 12;
/// The most a speck's ground may spread, over its rise: under dust the image is even at its scale, where grain,
/// a dense sky's noise and a patch of sky among leaves spread their ground as far as they rise.
const EVEN: f32 = 0.5;
/// The least a speck's fainter stretch rises, over half its peak's rise: on a hair 400 pixels long, rising 0.11 over
/// a grain of 0.04, the half alone holds 45 % of its pixels in pieces, this 86 % along one walk.
const FAINT: f32 = 0.5;

/// Each channel's median density over `cells`, 0 where there are none.
fn medians(cells: impl Iterator<Item = usize>, pixels: &[[f32; 4]], limits: &Limits) -> [f32; 3] {
    let mut lists: [Vec<f32>; 3] = Default::default();
    for i in cells {
        for (c, list) in lists.iter_mut().enumerate() {
            list.push(limits.density(&pixels[i], c));
        }
    }
    lists.map(|mut list| median(&mut list))
}

fn median(list: &mut [f32]) -> f32 {
    if list.is_empty() {
        return 0.0;
    }
    let mid = list.len() / 2;
    *list.select_nth_unstable_by(mid, f32::total_cmp).1
}

/// The ground under what `bounds` hold, read on the band `GROUND_AT` to `GROUND_AT + 2` pixels off them: each
/// channel's median density, and the widest channel's inter-quartile range, how uneven it is.
fn ground(bounds: [u32; 4], pixels: &[[f32; 4]], width: usize, limits: &Limits) -> ([f32; 3], f32) {
    let height = pixels.len() / width.max(1);
    let [x0, y0, x1, y1] = bounds.map(i64::from);
    let (near, far) = (i64::from(GROUND_AT), i64::from(GROUND_AT) + 2);
    let band = (y0 - far..y1 + far).flat_map(move |y| (x0 - far..x1 + far).map(move |x| (x, y))).filter(move |&(x, y)| x < x0 - near || x >= x1 + near || y < y0 - near || y >= y1 + near);
    let mut lists: [Vec<f32>; 3] = Default::default();
    for (x, y) in band.filter(|&(x, y)| x >= 0 && y >= 0 && x < width as i64 && y < height as i64) {
        for (c, list) in lists.iter_mut().enumerate() {
            list.push(limits.density(&pixels[y as usize * width + x as usize], c));
        }
    }
    let quartiles = lists.map(|mut list| {
        let n = list.len();
        if n < 4 {
            return [0.0; 3];
        }
        let (low, mid, high) = list.select_nth_unstable_by(n / 2, f32::total_cmp);
        let mid = *mid;
        let q1 = *low.select_nth_unstable_by(n / 4, f32::total_cmp).1;
        let q3 = *high.select_nth_unstable_by(n * 3 / 4 - n / 2 - 1, f32::total_cmp).1;
        [q1, mid, q3]
    });
    (quartiles.map(|q| q[1]), quartiles.iter().map(|q| q[2] - q[0]).fold(0.0, f32::max))
}

/// The eight steps `off` pixels long.
fn around(off: i64) -> [(i64, i64); 8] {
    [(off, 0), (-off, 0), (0, off), (0, -off), (off, off), (-off, off), (off, -off), (-off, -off)]
}

/// `seed` and the pixels joined to it that `above` holds, 8-connected; none once they pass `MAX_AREA`.
fn joined(seed: HashSet<usize>, above: impl Fn(usize) -> bool, width: usize, height: usize) -> Option<HashSet<usize>> {
    let mut object = seed;
    let mut queue: Vec<usize> = object.iter().copied().collect();
    while let Some(i) = queue.pop() {
        let (x, y) = ((i % width) as i64, (i / width) as i64);
        for (dx, dy) in around(1) {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= width as i64 || ny >= height as i64 {
                continue;
            }
            let n = ny as usize * width + nx as usize;
            if !object.contains(&n) && above(n) {
                if object.len() as u32 >= MAX_AREA {
                    return None;
                }
                object.insert(n);
                queue.push(n);
            }
        }
    }
    Some(object)
}

/// Pixels in reading order as a shape; none when it stands within `EDGE` of the frame's edge.
fn shape(sorted: &[usize], width: usize, height: usize) -> Option<Speck> {
    let mut runs: Vec<Run> = Vec::new();
    for &i in sorted {
        let (x, row) = ((i % width) as u32, (i / width) as u32);
        match runs.last_mut() {
            Some(r) if r.row == row && r.end == x => r.end += 1,
            _ => runs.push(Run { row, start: x, end: x + 1 }),
        }
    }
    let bounds = runs.iter().fold([u32::MAX, u32::MAX, 0, 0], |b, r| [b[0].min(r.start), b[1].min(r.row), b[2].max(r.end), b[3].max(r.row + 1)]);
    let inside = bounds[0] >= EDGE && bounds[1] >= EDGE && bounds[2] + EDGE <= width as u32 && bounds[3] + EDGE <= height as u32;
    inside.then(|| Speck { area: sorted.len() as u32, runs, bounds })
}

fn sorted(object: &HashSet<usize>) -> Vec<usize> {
    let mut sorted: Vec<usize> = object.iter().copied().collect();
    sorted.sort_unstable();
    sorted
}

/// What a shape past the window shows, judged whole: the pixels joined to its peak that rise over its ground by
/// half the peak's rise or more, so a flake whose densest pixel alone passes is read entire, and patched entire,
/// and a glint is read alike at every inset. Its jump is the least channel's rise, kept on an even ground when its
/// edge makes `SHARP` of it within `JUMP_AT`.
pub fn speck(shown: &Speck, pixels: &[[f32; 4]], width: usize, limits: &Limits) -> Option<(Speck, f32)> {
    let height = pixels.len() / width.max(1);
    let cells: Vec<usize> = shown.runs.iter().flat_map(|r| (r.start..r.end).map(move |x| r.row as usize * width + x as usize)).collect();
    let (ground, uneven) = self::ground(shown.bounds, pixels, width, limits);
    // The least channel: an opaque speck's blue, under the orange mask, sinks to the sensor's floor and would rule a mean.
    let rise = |i: usize| (0..3).map(|c| limits.density(&pixels[i], c) - ground[c]).fold(f32::INFINITY, f32::min);
    let densest = medians(cells.iter().copied(), pixels, limits);
    let least = (0..3).map(|c| densest[c] - ground[c]).fold(f32::INFINITY, f32::min);
    // Grain rises over its ground by less than the least jump, and a patch of the image stands among the image's
    // own shapes: most shapes end here, unread further.
    if least < JUMPS.1 || uneven > EVEN * least {
        return None;
    }
    let half = cells.iter().map(|&i| rise(i)).fold(f32::NEG_INFINITY, f32::max) / 2.0;
    let object = joined(cells.iter().copied().filter(|&i| rise(i) >= half).collect(), |i| rise(i) >= half, width, height)?;
    let body = sorted(&object);
    let judged = shape(&body, width, height)?;
    // The speck judged on its own ground, read off it as off what showed it.
    let (ground, uneven) = self::ground(judged.bounds, pixels, width, limits);
    let medians = medians(body.iter().copied(), pixels, limits);
    let jump = (0..3).map(|c| medians[c] - ground[c]).fold(f32::INFINITY, f32::min);
    if jump < JUMPS.1 || uneven > EVEN * jump {
        return None;
    }
    let mut across: [Vec<f32>; 3] = Default::default();
    for &i in &body {
        let (x, y) = ((i % width) as i64, (i / width) as i64);
        for (dx, dy) in around(JUMP_AT) {
            let (nx, ny) = (x + dx, y + dy);
            let n = ny as usize * width + nx as usize;
            if nx < 0 || ny < 0 || nx >= width as i64 || ny >= height as i64 || object.contains(&n) {
                continue;
            }
            for (c, list) in across.iter_mut().enumerate() {
                list.push(limits.density(&pixels[i], c) - limits.density(&pixels[n], c));
            }
        }
    }
    let edge = across.map(|mut list| median(&mut list)).into_iter().fold(f32::INFINITY, f32::min);
    if edge < SHARP * jump {
        return None;
    }
    // Kept, it holds the fainter stretches joined to it, as a hair's rise wavers along it: among what showed it
    // alone, past which the image's own lights join in.
    let showed: HashSet<usize> = cells.iter().copied().collect();
    let whole = joined(object, |i| showed.contains(&i) && rise(i) >= FAINT * half, width, height).and_then(|whole| shape(&sorted(&whole), width, height));
    Some((whole.unwrap_or(judged), jump))
}
