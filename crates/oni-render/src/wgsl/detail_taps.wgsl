// The detail stage's taps: each integrates the Gaussian over its pixel as Core Image's blur does,
// and the truncated sum telescopes to one erf, which normalises it.

// The toe under the sharpening's stops: a bare logarithm dives past -16 near black.
const SHARPEN_TOE: f32 = 0.01;
const SQRT1_2: f32 = 0.70710678118654752;

// erf to 1.5e-7 (Abramowitz and Stegun 7.1.26); its arguments here are never negative zero.
fn detail_erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.3275911 * abs(x));
    let poly = ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t;
    let v = 1.0 - poly * exp(-x * x);
    return select(-v, v, x >= 0.0);
}

struct Taps { scale: f32, norm: f32, inner: f32 }

fn taps(sigma: f32, reach: f32) -> Taps {
    var t: Taps;
    t.scale = select(0.0, SQRT1_2 / sigma, sigma > 0.0);
    t.norm = select(0.0, 1.0 / detail_erf((reach + 0.5) * t.scale), sigma > 0.0);
    t.inner = detail_erf(0.5 * t.scale);
    return t;
}

fn tap_centre(t: Taps) -> f32 {
    return t.inner * t.norm;
}

fn tap_next(t: ptr<function, Taps>, k: i32) -> f32 {
    let outer = detail_erf((f32(k) + 0.5) * (*t).scale);
    let w = 0.5 * (outer - (*t).inner) * (*t).norm;
    (*t).inner = outer;
    return w;
}

