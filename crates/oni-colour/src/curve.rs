//! An ICC tone curve evaluated in f64 as the ICC writes it: a power is an exact power, so a
//! curve read at zero gives zero, where a fast power approximation returns 1e8 off.

use moxcms::ToneReprCurve;

#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    Identity,
    Gamma(f64),
    /// Samples on [0, 1], interpolated linearly and held at their ends.
    Sampled(Vec<f64>),
    /// `parametricCurveType` in its full form: `(a·x + b)^g + e` from `d` up, `c·x + f` below.
    Parametric { g: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64 },
}

impl Curve {
    /// `None` for a parameter count the ICC does not define.
    pub fn new(curve: &ToneReprCurve) -> Option<Curve> {
        Some(match curve {
            ToneReprCurve::Lut(v) if v.is_empty() => Curve::Identity,
            ToneReprCurve::Lut(v) if v.len() == 1 => Curve::Gamma(v[0] as f64 / 256.0),
            ToneReprCurve::Lut(v) => Curve::Sampled(v.iter().map(|&s| s as f64 / 65535.0).collect()),
            ToneReprCurve::Parametric(p) => {
                let p: Vec<f64> = p.iter().map(|&v| v as f64).collect();
                let (g, zero) = (*p.first()?, 0.0);
                match p.len() {
                    1 => Curve::Parametric { g, a: 1.0, b: 0.0, c: 0.0, d: 0.0, e: 0.0, f: 0.0 },
                    // Types 1 and 2 hold `c` (zero for type 1) under the root of `a·x + b`.
                    3 | 4 => {
                        let (a, b) = (p[1], p[2]);
                        if a == 0.0 {
                            return None;
                        }
                        let c = if p.len() == 4 { p[3] } else { zero };
                        Curve::Parametric { g, a, b, c: 0.0, d: -b / a, e: c, f: c }
                    }
                    5 => Curve::Parametric { g, a: p[1], b: p[2], c: p[3], d: p[4], e: 0.0, f: 0.0 },
                    7 => Curve::Parametric { g, a: p[1], b: p[2], c: p[3], d: p[4], e: p[5], f: p[6] },
                    _ => return None,
                }
            }
        })
    }

    /// The curve at `x`, its analytic forms continued past one, a sampled one held at its ends.
    pub fn eval(&self, x: f64) -> f64 {
        match self {
            Curve::Identity => x,
            Curve::Gamma(g) => x.max(0.0).powf(*g),
            Curve::Sampled(s) => sampled(s, x),
            Curve::Parametric { g, a, b, c, d, e, f } => {
                if x >= *d { (a * x + b).max(0.0).powf(*g) + e } else { c * x + f }
            }
        }
    }

    /// The input whose value is `y`; analytic where the ICC form allows, else found by halving.
    pub fn inverse(&self, y: f64) -> f64 {
        match self {
            Curve::Identity => y,
            Curve::Gamma(g) if *g > 0.0 => y.max(0.0).powf(1.0 / g),
            Curve::Parametric { g, a, b, c, d, e, f } if *g > 0.0 && *a > 0.0 && *c > 0.0 && *e == 0.0 && *f == 0.0 => {
                if y >= c * d { ((y.max(0.0)).powf(1.0 / g) - b) / a } else { y / c }
            }
            _ => {
                let (mut lo, mut hi) = (0.0, 1.0);
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    if self.eval(mid) < y { lo = mid } else { hi = mid }
                }
                0.5 * (lo + hi)
            }
        }
    }
}

/// A sampled curve on [0, 1], interpolated linearly.
pub fn sampled(table: &[f64], x: f64) -> f64 {
    let n = table.len();
    if n < 2 {
        return x;
    }
    let at = x.clamp(0.0, 1.0) * (n - 1) as f64;
    let lo = (at.floor() as usize).min(n - 2);
    let t = at - lo as f64;
    table[lo] + (table[lo + 1] - table[lo]) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_power_reads_zero_at_zero_and_inverts() {
        let gamma = Curve::new(&ToneReprCurve::Lut(vec![461])).unwrap();
        assert_eq!(gamma.eval(0.0), 0.0);
        assert!((gamma.eval(0.5) - 0.5f64.powf(461.0 / 256.0)).abs() < 1e-12);
        let srgb = Curve::new(&ToneReprCurve::Parametric(vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045])).unwrap();
        for x in [0.0, 0.002, 0.04, 0.3, 1.0] {
            assert!((srgb.inverse(srgb.eval(x)) - x).abs() < 1e-6 && (gamma.inverse(gamma.eval(x)) - x).abs() < 1e-9, "{x}");
        }
        assert!((srgb.eval(1.01) - 1.0).abs() > 0.02, "an analytic curve continues past one");
        // Twin: the fast power the CMM's own evaluator uses reads a power at zero far off zero.
        let fast = ToneReprCurve::Lut(vec![461]).make_linear_evaluator().unwrap().evaluate_value(0.0);
        assert!(fast.abs() > 1.0, "{fast}");
    }

    #[test]
    fn the_icc_parameter_counts_name_their_types() {
        let at = |p: Vec<f32>, x: f64| Curve::new(&ToneReprCurve::Parametric(p)).unwrap().eval(x);
        assert!((at(vec![2.0, 1.0, -0.2], 0.1) - 0.0).abs() < 1e-7, "type 1 is zero under -b/a");
        assert!((at(vec![2.0, 1.0, -0.2, 0.05], 0.1) - 0.05).abs() < 1e-7, "type 2 holds c under it");
        assert!((at(vec![1.0, 1.0, 0.0, 0.5, 0.2, 0.1, 0.3], 0.1) - 0.35).abs() < 1e-7, "type 4 is c·x + f below d");
        // Twin: a sampled curve is held at its last sample past one.
        let samples = Curve::new(&ToneReprCurve::Lut(vec![0, 32768, 65535])).unwrap();
        assert_eq!(samples.eval(1.5), 1.0);
        assert!(Curve::new(&ToneReprCurve::Parametric(vec![2.0, 1.0])).is_none());
    }
}
