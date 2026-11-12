//! The least squares the lighting fit runs: a damped Gauss-Newton on bounded parameters and the
//! elimination each of its steps solves.

pub(super) type Bounds = (f32, f32);

/// A damped Gauss-Newton (Levenberg-Marquardt) on bounded parameters, with a ridge pulling each to zero.
pub(super) fn minimised(start: Vec<f32>, bounds: &[Bounds], ridge: &[f32], residuals: &dyn Fn(&[f32]) -> Vec<f32>) -> Vec<f32> {
    let cost = |r: &[f32], p: &[f32]| -> f32 {
        let a = r.iter().fold(0.0f32, |s, v| s + v * v);
        p.iter().zip(ridge).fold(a, |s, (p, k)| s + p * p * k * k)
    };
    let all = |p: &[f32]| -> Vec<f32> {
        let mut r = residuals(p);
        r.extend(p.iter().zip(ridge).map(|(p, k)| p * k));
        r
    };
    let mut x = start;
    let mut r = all(&x);
    let mut current = cost(&residuals(&x), &x);
    let mut lambda: f32 = 1e-3;
    for _ in 0..80 {
        let n = x.len();
        let m = r.len();
        let mut jac = vec![0f32; m * n];
        for j in 0..n {
            let mut p = x.clone();
            let h = (1e-3 * (bounds[j].1 - bounds[j].0)).max(1e-5);
            p[j] = x[j] + h;
            let rj = all(&p);
            for i in 0..m {
                jac[i * n + j] = (rj[i] - r[i]) / h;
            }
        }
        let mut jtj = vec![0f32; n * n];
        let mut jtr = vec![0f32; n];
        for i in 0..m {
            for a in 0..n {
                let ja = jac[i * n + a];
                jtr[a] += ja * r[i];
                for b in 0..n {
                    jtj[a * n + b] += ja * jac[i * n + b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..8 {
            let mut a = jtj.clone();
            for d in 0..n {
                a[d * n + d] *= 1.0 + lambda;
                a[d * n + d] += 1e-9;
            }
            let b: Vec<f32> = jtr.iter().map(|v| -v).collect();
            let Some(step) = solved(a, b, n) else {
                lambda *= 10.0;
                continue;
            };
            let trial: Vec<f32> =
                (0..n).map(|j| (x[j] + step[j]).max(bounds[j].0).min(bounds[j].1)).collect();
            let c = cost(&residuals(&trial), &trial);
            if c < current {
                let gain = current - c;
                x = trial;
                current = c;
                r = all(&x);
                lambda = (lambda / 3.0).max(1e-7);
                improved = true;
                if gain < 1e-10 {
                    return x;
                }
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    x
}

/// Gaussian elimination with partial pivoting; ties keep the first row, as the reference does.
fn solved(mut m: Vec<f32>, mut v: Vec<f32>, n: usize) -> Option<Vec<f32>> {
    for c in 0..n {
        let mut p = c;
        for row in (c + 1)..n {
            if m[p * n + c].abs() < m[row * n + c].abs() {
                p = row;
            }
        }
        if m[p * n + c].abs() <= 1e-12 {
            return None;
        }
        if p != c {
            for k in 0..n {
                m.swap(c * n + k, p * n + k);
            }
            v.swap(c, p);
        }
        for r in (c + 1)..n {
            let f = m[r * n + c] / m[c * n + c];
            if f == 0.0 {
                continue;
            }
            for k in c..n {
                m[r * n + k] -= f * m[c * n + k];
            }
            v[r] -= f * v[c];
        }
    }
    let mut x = vec![0f32; n];
    for r in (0..n).rev() {
        let mut s = v[r];
        for k in (r + 1)..n {
            s -= m[r * n + k] * x[k];
        }
        x[r] = s / m[r * n + r];
    }
    Some(x)
}
