//! The paper curve: a monotone cubic through fixed density nodes (Fritsch–Carlson), so a graph of its
//! nodes draws exactly what renders.

/// The curve's nodes: 15 densities from `FIRST` to `LAST`, evenly spaced.
pub const NODES: usize = 15;
pub const FIRST: f64 = -0.2;
pub const LAST: f64 = 2.0;

pub fn density(i: usize) -> f64 {
    FIRST + (LAST - FIRST) * i as f64 / (NODES - 1) as f64
}

/// The tangents at each node: the secants' mean, limited so no interval overshoots its ends.
fn tangents(y: &[f64; NODES]) -> [f64; NODES] {
    let h = (LAST - FIRST) / (NODES - 1) as f64;
    let secant: [f64; NODES - 1] = std::array::from_fn(|k| (y[k + 1] - y[k]) / h);
    let mut m = [0.0; NODES];
    m[0] = secant[0];
    m[NODES - 1] = secant[NODES - 2];
    for k in 1..NODES - 1 {
        m[k] = if secant[k - 1] * secant[k] <= 0.0 { 0.0 } else { (secant[k - 1] + secant[k]) / 2.0 };
    }
    for k in 0..NODES - 1 {
        if secant[k] == 0.0 {
            (m[k], m[k + 1]) = (0.0, 0.0);
            continue;
        }
        let (a, b) = (m[k] / secant[k], m[k + 1] / secant[k]);
        let r = a * a + b * b;
        if r > 9.0 {
            let t = 3.0 / r.sqrt();
            (m[k], m[k + 1]) = (t * a * secant[k], t * b * secant[k]);
        }
    }
    m
}

/// The curve with its tangents laid once, read many times by a bake.
pub struct Curve {
    y: [f64; NODES],
    m: [f64; NODES],
}

impl Curve {
    pub fn new(y: &[f64; NODES]) -> Self {
        Curve { y: *y, m: tangents(y) }
    }

    /// The curve at density `x`, held at its end values outside the nodes.
    pub fn at(&self, x: f64) -> f64 {
        let y = &self.y;
        if x <= FIRST {
            return y[0];
        }
        if x >= LAST {
            return y[NODES - 1];
        }
        let h = (LAST - FIRST) / (NODES - 1) as f64;
        let k = (((x - FIRST) / h) as usize).min(NODES - 2);
        let t = (x - density(k)) / h;
        let (t2, t3) = (t * t, t * t * t);
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        h00 * y[k] + h10 * h * self.m[k] + h01 * y[k + 1] + h11 * h * self.m[k + 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_passes_its_nodes_and_never_turns_back() {
        let y = [0.0, 0.0, 0.139, 0.341, 0.519, 0.659, 0.771, 0.860, 0.929, 0.984, 1.0, 1.0, 1.0, 1.0, 1.0];
        let curve = Curve::new(&y);
        for i in 0..NODES {
            assert!((curve.at(density(i)) - y[i]).abs() < 1e-12);
        }
        let samples: Vec<f64> = (0..=2200).map(|i| curve.at(FIRST + i as f64 * 1e-3)).collect();
        assert!(samples.windows(2).all(|w| w[1] >= w[0] - 1e-12), "monotone between the nodes");
        assert!(samples.iter().all(|&v| (0.0..=1.0).contains(&v)), "no overshoot past the flat top");
        // Twin: the plain Catmull-Rom tangents climb past the flat top after 0.984.
        let h = (LAST - FIRST) / (NODES - 1) as f64;
        let (k, t) = (10, 0.5);
        let m = |i: usize| (y[i + 1] - y[i - 1]) / (2.0 * h);
        let plain = (2.0 * t * t * t - 3.0 * t * t + 1.0) * y[k]
            + (t * t * t - 2.0 * t * t + t) * h * m(k)
            + (-2.0 * t * t * t + 3.0 * t * t) * y[k + 1]
            + (t * t * t - t * t) * h * m(k + 1);
        assert!(plain > 1.0, "{plain}");
    }
}
