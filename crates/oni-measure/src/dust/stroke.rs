//! A speck as a brush stroke: along its longest walk, so a patch follows a hair's curve and covers no more than it must.

use super::Speck;
use oni_settings::frame::Correction;
use std::collections::{HashMap, VecDeque};

/// What a patch covers past a shape's pixels, in pixels: a speck's soft edge lies inside the window, and 4 left its ghost.
pub(super) const HALO: f64 = 8.0;
/// The path's pixels between two points of a stroke, enough to follow a hair's curve.
const STEP: usize = 6;

/// The two ends of a shape's longest walk and the walk between them, the shape's own pixels only.
fn longest_walk(cells: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let index: HashMap<(i32, i32), usize> = cells.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let walk = |from: usize| {
        let mut before = vec![usize::MAX; cells.len()];
        before[from] = from;
        let mut queue = VecDeque::from([from]);
        let mut last = from;
        while let Some(i) = queue.pop_front() {
            last = i;
            let (x, y) = cells[i];
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
                if let Some(&n) = index.get(&(x + dx, y + dy))
                    && before[n] == usize::MAX
                {
                    before[n] = i;
                    queue.push_back(n);
                }
            }
        }
        (last, before)
    };
    let (start, _) = walk(0);
    let (end, before) = walk(start);
    let mut path = vec![cells[end]];
    let mut at = end;
    while at != start {
        at = before[at];
        path.push(cells[at]);
    }
    path
}

pub(super) fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l <= 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// A speck as a brush stroke, in fractions of the frame, y down: along its longest walk, wide enough for
/// every pixel it holds and its soft edge, inside the brush's range.
pub fn stroke(s: &Speck, width: usize, height: usize) -> (Vec<(f64, f64)>, f64) {
    let cells: Vec<(i32, i32)> = s.runs.iter().flat_map(|r| (r.start..r.end).map(move |x| (x as i32, r.row as i32))).collect();
    let centre = |c: (i32, i32)| (f64::from(c.0) + 0.5, f64::from(c.1) + 0.5);
    let path = longest_walk(&cells);
    let mut points: Vec<(f64, f64)> = path.iter().step_by(STEP).map(|&c| centre(c)).collect();
    let last = centre(*path.last().expect("a shape holds a pixel"));
    if points.last() != Some(&last) {
        points.push(last);
    }
    // Two points at least, as a hand's stroke holds: a lone pixel's stroke is half a pixel long.
    if points.len() == 1 {
        points.push((last.0 + 0.5, last.1));
    }
    let reach = cells.iter().fold(0.0f64, |most, &c| most.max(points.windows(2).map(|w| distance_to_segment(centre(c), w[0], w[1])).fold(f64::MAX, f64::min)));
    let (low, high) = Correction::RADIUS_RANGE;
    let radius = (reach + HALO).clamp(low, high);
    (points.iter().map(|p| (p.0 / width as f64, p.1 / height as f64)).collect(), radius)
}
