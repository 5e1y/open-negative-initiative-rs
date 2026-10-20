//! Levenberg–Marquardt over the Lab's values, for the fit tool: a set flattened to a vector with
//! each value's range, its derivative step, and the paper kept monotone.

use super::measure::lab_of;
use super::paper::NODES;
use super::{Chain, GREYS, LabParams, SECTORS};

/// How many values a set holds once flattened; the character is not fitted.
pub const COUNT: usize = 6 + NODES + 2 + 2 + 3 * SECTORS + 1 + 2 * GREYS;

/// Each value's range and derivative step, in the flattened order.
pub fn ranges() -> Vec<(f64, f64, f64)> {
    let mut out = Vec::with_capacity(COUNT);
    out.extend([(-0.3, 0.3, 1e-5); 6]);
    out.extend([(0.0, 1.0, 1e-5); NODES]);
    out.extend([(0.8, 1.2, 1e-5); 2]);
    out.extend([(-0.2, 0.2, 1e-5); 2]);
    out.extend([(-20.0, 20.0, 1e-3); SECTORS]);
    out.extend([(-15.0, 15.0, 1e-3); SECTORS]);
    out.extend([(0.5, 1.5, 1e-5); SECTORS]);
    out.push((-0.3, 0.3, 1e-5));
    out.extend([(-0.03, 0.03, 1e-6); 2 * GREYS]);
    out
}

pub fn pack(p: &LabParams) -> Vec<f64> {
    let mut v = Vec::with_capacity(COUNT);
    v.extend(p.matrix);
    v.extend(p.paper);
    v.extend(p.paper_gain);
    v.extend(p.paper_offset);
    v.extend(p.hue);
    v.extend(p.twist);
    v.extend(p.chroma);
    v.push(p.saturation_by_l);
    v.extend(p.grey_a);
    v.extend(p.grey_b);
    v
}

pub fn unpack(v: &[f64], character: f64) -> LabParams {
    let mut at = 0;
    let mut take = |n: usize| {
        at += n;
        &v[at - n..at]
    };
    LabParams {
        matrix: take(6).try_into().unwrap(),
        paper: take(NODES).try_into().unwrap(),
        paper_gain: take(2).try_into().unwrap(),
        paper_offset: take(2).try_into().unwrap(),
        hue: take(SECTORS).try_into().unwrap(),
        twist: take(SECTORS).try_into().unwrap(),
        chroma: take(SECTORS).try_into().unwrap(),
        saturation_by_l: take(1)[0],
        grey_a: take(GREYS).try_into().unwrap(),
        grey_b: take(GREYS).try_into().unwrap(),
        character,
    }
}

/// Every value onto its range, the paper's nodes never falling from one to the next.
pub fn project(v: &mut [f64]) {
    for (x, (lo, hi, _)) in v.iter_mut().zip(ranges()) {
        *x = x.clamp(lo, hi);
    }
    for i in 7..6 + NODES {
        v[i] = v[i].max(v[i - 1]);
    }
}

/// The Oklab gap × 100 at each sample, three entries per (density, target code) pair.
pub fn residuals(v: &[f64], samples: &[([f64; 3], [f64; 3])]) -> Vec<f64> {
    let chain = Chain::new(&unpack(v, 0.0));
    samples
        .iter()
        .flat_map(|(x, target)| {
            let (m, t) = (lab_of(chain.code(*x)), lab_of(*target));
            [0, 1, 2].map(|c| 100.0 * (m[c] - t[c]))
        })
        .collect()
}

fn cost(r: &[f64]) -> f64 {
    r.iter().map(|x| x * x).sum()
}

/// Forward differences, the values shared out over the machine's cores.
fn jacobian(v: &[f64], r: &[f64], samples: &[([f64; 3], [f64; 3])]) -> Vec<Vec<f64>> {
    let steps: Vec<f64> = ranges().iter().map(|r| r.2).collect();
    let mut columns = vec![Vec::new(); v.len()];
    let workers = std::thread::available_parallelism().map_or(1, |w| w.get());
    let per = v.len().div_ceil(workers);
    std::thread::scope(|scope| {
        for (k, part) in columns.chunks_mut(per).enumerate() {
            let steps = &steps;
            scope.spawn(move || {
                for (i, column) in part.iter_mut().enumerate() {
                    let j = k * per + i;
                    let mut moved = v.to_vec();
                    moved[j] += steps[j];
                    *column = residuals(&moved, samples).iter().zip(r).map(|(a, b)| (a - b) / steps[j]).collect();
                }
            });
        }
    });
    columns
}

/// `a x = b` for a symmetric positive definite `a`, `None` when it is not.
fn cholesky_solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = a[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            if i == j {
                if s <= 0.0 {
                    return None;
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    let mut y = vec![0.0; n];
    for i in 0..n {
        y[i] = (b[i] - (0..i).map(|k| l[i][k] * y[k]).sum::<f64>()) / l[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (y[i] - (i + 1..n).map(|k| l[k][i] * x[k]).sum::<f64>()) / l[i][i];
    }
    Some(x)
}

/// The values from `start` that bring the chain closest to the targets, in at most `rounds` steps.
pub fn fit(start: &LabParams, samples: &[([f64; 3], [f64; 3])], rounds: usize, report: impl Fn(usize, f64)) -> LabParams {
    let mut v = pack(start);
    project(&mut v);
    let mut r = residuals(&v, samples);
    let mut now = cost(&r);
    let mut lambda = 1e-3;
    for round in 0..rounds {
        let j = jacobian(&v, &r, samples);
        let n = v.len();
        let a: Vec<Vec<f64>> = (0..n).map(|p| (0..n).map(|q| j[p].iter().zip(&j[q]).map(|(x, y)| x * y).sum()).collect()).collect();
        let g: Vec<f64> = (0..n).map(|p| -j[p].iter().zip(&r).map(|(x, y)| x * y).sum::<f64>()).collect();
        // A value on its stop that the descent pushes further out stays there this round.
        let bounds = ranges();
        let free: Vec<usize> = (0..n).filter(|&p| !((v[p] <= bounds[p].0 && g[p] < 0.0) || (v[p] >= bounds[p].1 && g[p] > 0.0))).collect();
        let mut better = false;
        for _ in 0..12 {
            let damped: Vec<Vec<f64>> = free
                .iter()
                .map(|&p| free.iter().map(|&q| if p == q { a[p][q] * (1.0 + lambda) + 1e-9 } else { a[p][q] }).collect())
                .collect();
            let Some(solved) = cholesky_solve(&damped, &free.iter().map(|&p| g[p]).collect::<Vec<_>>()) else {
                lambda *= 4.0;
                continue;
            };
            let mut step = vec![0.0; n];
            free.iter().zip(&solved).for_each(|(&p, d)| step[p] = *d);
            let mut tried: Vec<f64> = v.iter().zip(&step).map(|(x, d)| x + d).collect();
            project(&mut tried);
            let tried_r = residuals(&tried, samples);
            let then = cost(&tried_r);
            if then < now {
                better = now - then > 1e-7 * now;
                (v, r, now) = (tried, tried_r, then);
                lambda = (lambda / 3.0).max(1e-9);
                break;
            }
            lambda *= 4.0;
        }
        report(round, now);
        if !better {
            break;
        }
    }
    unpack(&v, start.character)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look_lab::SEED_FRONTIER;

    #[test]
    fn a_set_flattens_and_comes_back_whole() {
        let v = pack(&SEED_FRONTIER);
        assert_eq!((v.len(), ranges().len()), (COUNT, COUNT));
        assert_eq!(unpack(&v, 1.0), SEED_FRONTIER);
        // Twin: the character is not carried by the vector, so another one comes back another set.
        assert_ne!(unpack(&v, 0.0), SEED_FRONTIER);
    }

    #[test]
    fn the_paper_never_falls_and_values_hold_their_ranges() {
        let mut v = pack(&SEED_FRONTIER);
        v[6 + 8] = 0.1;
        v[0] = 2.0;
        project(&mut v);
        assert!(v[6..6 + NODES].windows(2).all(|w| w[1] >= w[0]));
        assert_eq!(v[0], 0.3);
        // Twin: before the projection the same values fall.
        let mut raw = pack(&SEED_FRONTIER);
        raw[6 + 8] = 0.1;
        assert!(!raw[6..6 + NODES].windows(2).all(|w| w[1] >= w[0]));
    }

    #[test]
    fn the_fit_recovers_a_moved_value() {
        let target = LabParams { matrix: [0.08, 0.024, 0.100, -0.023, 0.011, 0.233], ..SEED_FRONTIER.paper_only() };
        let truth = Chain::new(&target);
        let samples: Vec<_> = super::super::measure::constrained().iter().map(|(x, _)| (*x, truth.code(*x))).collect();
        let found = fit(&SEED_FRONTIER.paper_only(), &samples, 30, |_, _| {});
        let gap = cost(&residuals(&pack(&found), &samples)) / samples.len() as f64;
        assert!(gap < 1e-4, "mean squared gap {gap}");
        // Twin: the start itself sits away from the targets.
        let start = cost(&residuals(&pack(&SEED_FRONTIER.paper_only()), &samples)) / samples.len() as f64;
        assert!(start > 1e-2, "{start}");
    }
}
