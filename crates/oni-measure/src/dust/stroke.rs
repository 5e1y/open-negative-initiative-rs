//! A speck as brush strokes: along its longest walks, so a patch follows a hair's curve and covers no more than it must.

use super::Speck;
use oni_settings::frame::Correction;
use std::collections::{HashMap, VecDeque};

/// What a patch covers past a shape's pixels, in pixels: a speck's soft edge lies inside the window, and 4 left its ghost.
pub(super) const HALO: f64 = 8.0;
/// The path's pixels between two points of a stroke, enough to follow a hair's curve.
const STEP: usize = 6;
/// A hair's own stroke, in pixels of radius: what its walk is first cut by, before a wider part joins its neighbours.
const THIN: f64 = HALO + 2.0;

const AROUND: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];

/// The longest walk through the pixels of `cells` still `open` that are joined to `from`, end to end.
fn longest_walk(cells: &[(i32, i32)], index: &HashMap<(i32, i32), usize>, open: &[bool], from: usize) -> Vec<(i32, i32)> {
    let walk = |from: usize| {
        let mut before = vec![usize::MAX; cells.len()];
        before[from] = from;
        let mut queue = VecDeque::from([from]);
        let mut last = from;
        while let Some(i) = queue.pop_front() {
            last = i;
            let (x, y) = cells[i];
            for (dx, dy) in AROUND {
                if let Some(&n) = index.get(&(x + dx, y + dy))
                    && open[n]
                    && before[n] == usize::MAX
                {
                    before[n] = i;
                    queue.push_back(n);
                }
            }
        }
        (last, before)
    };
    let (start, _) = walk(from);
    let (end, before) = walk(start);
    let mut path = vec![cells[end]];
    let mut at = end;
    while at != start {
        at = before[at];
        path.push(cells[at]);
    }
    path
}

/// How deep inside the shape its innermost pixel lies, in pixels: half a hair's width, a blob's radius.
fn thickness(cells: &[(i32, i32)], index: &HashMap<(i32, i32), usize>) -> f64 {
    let mut depth = vec![0u32; cells.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &(x, y)) in cells.iter().enumerate() {
        if AROUND.iter().any(|(dx, dy)| !index.contains_key(&(x + dx, y + dy))) {
            depth[i] = 1;
            queue.push_back(i);
        }
    }
    let mut deepest = 1;
    while let Some(i) = queue.pop_front() {
        let (x, y) = cells[i];
        for (dx, dy) in AROUND {
            if let Some(&n) = index.get(&(x + dx, y + dy))
                && depth[n] == 0
            {
                depth[n] = depth[i] + 1;
                deepest = deepest.max(depth[n]);
                queue.push_back(n);
            }
        }
    }
    f64::from(deepest)
}

pub(super) fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l <= 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

fn distance_to_path(p: (f64, f64), path: &[(f64, f64)]) -> f64 {
    path.windows(2).map(|w| distance_to_segment(p, w[0], w[1])).fold(f64::MAX, f64::min)
}

fn span(parts: &[&[(f64, f64)]]) -> f64 {
    let b = parts.iter().flat_map(|p| p.iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
    (b[2] - b[0]).max(b[3] - b[1])
}

/// A speck as brush strokes with their radii, in fractions of the frame, y down: along its longest walk, then what
/// that leaves, a branch or a loop's other side, each cut as a hand's and as wide as the pixels beside it and their soft edge.
pub fn strokes(s: &Speck, width: usize, height: usize) -> Vec<(Vec<(f64, f64)>, f64)> {
    let cells: Vec<(i32, i32)> = s.runs.iter().flat_map(|r| (r.start..r.end).map(move |x| (x as i32, r.row as i32))).collect();
    let index: HashMap<(i32, i32), usize> = cells.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let centre = |c: (i32, i32)| (f64::from(c.0) + 0.5, f64::from(c.1) + 0.5);
    // A walk holds the pixels this near it: a blob's whole width, and of a hair no more than its soft edge.
    let near = (2.0 * thickness(&cells, &index)).max(HALO);
    let (low, high) = Correction::RADIUS_RANGE;
    let mut open = vec![true; cells.len()];
    let mut out = Vec::new();
    while let Some(from) = open.iter().position(|&o| o) {
        let path = longest_walk(&cells, &index, &open, from);
        let mut points: Vec<(f64, f64)> = path.iter().step_by(STEP).map(|&c| centre(c)).collect();
        let last = centre(*path.last().expect("a walk holds a pixel"));
        if points.last() != Some(&last) {
            points.push(last);
        }
        // Two points at least, as a hand's stroke holds: a lone pixel's stroke is half a pixel long.
        if points.len() == 1 {
            points.push((last.0 + 0.5, last.1));
        }
        let cut = Correction::parts(&points, Correction::SPAN * THIN);
        // Each part reaches the farthest of the walk's pixels that lie nearest it.
        let mut reach = vec![0.0f64; cut.len()];
        for (open, &cell) in open.iter_mut().zip(&cells).filter(|(open, _)| **open) {
            let distances = cut.iter().map(|part| distance_to_path(centre(cell), part));
            let (nearest, distance) = distances.enumerate().fold((0, f64::MAX), |best, (k, d)| if d < best.1 { (k, d) } else { best });
            if distance <= near {
                *open = false;
                reach[nearest] = reach[nearest].max(distance);
            }
        }
        // A wide part, a blob or a hair's clump, takes in its neighbours while they span no more than its own stroke may.
        let mut parts: Vec<(Vec<(f64, f64)>, f64)> = Vec::new();
        for (part, reach) in cut.into_iter().zip(reach) {
            let radius = (reach + HALO).clamp(low, high);
            match parts.last_mut() {
                Some((before, wide)) if span(&[before, &part]) <= Correction::SPAN * wide.max(radius) => {
                    before.extend(part.into_iter().skip(1));
                    *wide = wide.max(radius);
                }
                _ => parts.push((part, radius)),
            }
        }
        out.extend(parts.into_iter().map(|(part, radius)| (part.iter().map(|p| (p.0 / width as f64, p.1 / height as f64)).collect(), radius)));
    }
    out
}
